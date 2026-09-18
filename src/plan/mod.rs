// SPDX-License-Identifier: GPL-3.0-only

//! The hybrid mount planner.
//!
//! Rule precedence: path rules, then the module `default_mode`, then the global `default_mode`.
//! A file path goes to exactly one backend; plain structural directories can be shared, and a real conflict is an explicit boot error.
//! Output:
//! - overlay operations grouped by target partition, with directory rules used directly as lowerdirs
//!   and file rules left to the executor as shallow layers (v4.2.0 prepare semantics);
//! - one backend-labelled node tree handed straight to the OverlayFS and Magic Mount executors.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::config::{Config, Mode};
use crate::errors::{Error, Result};
use crate::module_id::ModuleId;
use crate::mount_tree::{MountNode, MountSource, MountTree, NodeFileType};
use crate::scanner::{ModuleEntry, ModuleRecord};

/// One overlay mount operation, structured like v4.2.0's `OverlayOperation`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayOperation {
    pub partition: String,
    pub target: String,
    pub lowerdirs: Vec<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MountPlan {
    /// The single node tree produced by the scanner and planner and consumed by both executors.
    pub tree: MountTree,
    /// Directory-level overlay: target mountpoint to ordered lowerdirs, sorted by module id.
    pub overlay_ops: Vec<OverlayOperation>,
    /// File-level overlay rules: parent directory target to file sources, which the executor turns into shallow layers.
    pub overlay_files: BTreeMap<String, Vec<PathBuf>>,
    pub overlay_module_ids: Vec<ModuleId>,
    pub magic_module_ids: Vec<ModuleId>,
    pub vfs_module_ids: Vec<ModuleId>,
}

pub struct PlanInput<'a> {
    pub modules: &'a [ModuleRecord],
    pub config: &'a Config,
    /// Promoted partitions discovered by the executor from the built-in promotion rules.
    pub promoted_partitions: &'a BTreeSet<String>,
    /// Whether the kernel provides the VFS backend this boot. When false, every `vfs`
    /// rule degrades to `ignore` so no plan, count or page can advertise a backend the
    /// executor cannot use.
    pub vfs_available: bool,
}

/// Builds the mount plan; cross-backend file, type and `.replace` conflicts are errors.
pub fn build_plan(input: &PlanInput<'_>) -> Result<MountPlan> {
    let mut modules: Vec<&ModuleRecord> = input.modules.iter().collect();
    modules.sort_by(|left, right| left.id.cmp(&right.id));

    let mut builder = PlanBuilder::default();
    for module in modules {
        if !module.mountable() {
            continue;
        }
        if input.config.is_module_blacklisted(module.id.as_str()) {
            log::debug!("plan skip module: id={}, reason=blacklisted", module.id);
            continue;
        }
        let rules = ModuleRulesView::new(&module.id, input.config, input.vfs_available);
        process_module(module, &rules, input.promoted_partitions, &mut builder)?;
    }

    ensure_replace_backend_consistency(&builder.tree.root, "")?;
    ensure_vfs_not_shadowed(&builder.tree.root, "", None)?;
    ensure_vfs_opaque_exclusive(&builder.tree.root, "", None)?;
    Ok(builder.finish())
}

struct ModuleRulesView {
    default_mode: Mode,
    /// `(normalized_key, mode)`, sorted by key length descending so the longest prefix wins.
    path_rules: Vec<(String, Mode)>,
}

impl ModuleRulesView {
    fn new(module_id: &ModuleId, config: &Config, vfs_available: bool) -> Self {
        let module_rule = config.rules.get(module_id);
        let default_mode = module_rule
            .and_then(|rule| rule.default_mode)
            .unwrap_or(config.default_mode);

        let mut path_rules: Vec<(String, Mode)> = module_rule
            .map(|rule| {
                rule.paths
                    .iter()
                    .map(|(key, mode)| (normalize_rule_path(key), *mode))
                    .collect()
            })
            .unwrap_or_default();
        path_rules.sort_by(|left, right| {
            right
                .0
                .len()
                .cmp(&left.0.len())
                .then_with(|| left.0.cmp(&right.0))
        });

        // A kernel without the module has no VFS backend to send rules to, so a configured `vfs`
        // rule resolves to `ignore` instead of producing a plan the executor cannot run.
        let degrade = |mode: Mode| {
            if mode == Mode::Vfs && !vfs_available {
                Mode::Ignore
            } else {
                mode
            }
        };

        Self {
            default_mode: degrade(default_mode),
            path_rules: path_rules
                .into_iter()
                .map(|(key, mode)| (key, degrade(mode)))
                .collect(),
        }
    }

    fn resolve_mode(&self, relative: &str) -> Mode {
        self.path_rules
            .iter()
            .find(|(key, _)| crate::utils::is_same_or_below(relative, key))
            .map(|(_, mode)| *mode)
            .unwrap_or(self.default_mode)
    }

    /// Whether a path rule classifies the whole `system` as overlay.
    fn has_whole_system_overlay_rule(&self) -> bool {
        self.path_rules
            .iter()
            .any(|(key, mode)| key == "system" && *mode == Mode::Overlay)
    }
}

#[derive(Default)]
struct PlanBuilder {
    tree: MountTree,
    /// target -> (partition, module id -> lowerdir)
    overlay_by_target: BTreeMap<String, (String, BTreeMap<ModuleId, PathBuf>)>,
    /// File rules: parent directory target to ordered, deduplicated (module id, file source).
    /// One module may contribute several files to the same parent directory.
    overlay_files_by_target: BTreeMap<String, BTreeSet<(ModuleId, PathBuf)>>,
    overlay_module_ids: BTreeSet<ModuleId>,
    magic_module_ids: BTreeSet<ModuleId>,
    vfs_module_ids: BTreeSet<ModuleId>,
    /// Cross-module allocation table from target to already-assigned node, used for conflict detection.
    assignments: BTreeMap<String, Vec<TargetAssignment>>,
}

struct TargetAssignment {
    mode: Mode,
    source: String,
    file_type: NodeFileType,
    replace: bool,
}

impl PlanBuilder {
    fn register(
        &mut self,
        target: &str,
        mode: Mode,
        module_id: &ModuleId,
        relative: &str,
        file_type: NodeFileType,
        replace: bool,
    ) -> Result<()> {
        let source = format!(
            "{module_id}:{relative}{}",
            if replace { " (.replace)" } else { "" }
        );
        let assignments = self.assignments.entry(target.to_owned()).or_default();
        for existing in assignments.iter() {
            let shareable_directories = existing.file_type == NodeFileType::Directory
                && file_type == NodeFileType::Directory
                && !existing.replace
                && !replace;
            if existing.mode != mode && !shareable_directories {
                return Err(Error::PlanConflict {
                    target: target.to_owned(),
                    first_backend: existing.mode.as_str().to_owned(),
                    first_source: existing.source.clone(),
                    second_backend: mode.as_str().to_owned(),
                    second_source: source,
                });
            }
        }
        assignments.push(TargetAssignment {
            mode,
            source,
            file_type,
            replace,
        });
        Ok(())
    }

    fn add_overlay_layer(
        &mut self,
        partition: &str,
        target: &str,
        module_id: &ModuleId,
        source: PathBuf,
    ) {
        let (_, layers) = self
            .overlay_by_target
            .entry(target.to_owned())
            .or_insert_with(|| (partition.to_owned(), BTreeMap::new()));
        layers.insert(module_id.clone(), source);
        self.overlay_module_ids.insert(module_id.clone());
    }

    fn add_overlay_file(&mut self, target: &str, module_id: &ModuleId, source: PathBuf) {
        self.overlay_files_by_target
            .entry(target.to_owned())
            .or_default()
            .insert((module_id.clone(), source));
        self.overlay_module_ids.insert(module_id.clone());
    }

    fn finish(self) -> MountPlan {
        let overlay_ops = self
            .overlay_by_target
            .into_iter()
            .map(|(target, (partition, layers))| OverlayOperation {
                partition,
                target,
                lowerdirs: layers.into_values().collect(),
            })
            .collect();

        let overlay_files = self
            .overlay_files_by_target
            .into_iter()
            .map(|(target, sources)| {
                (
                    target,
                    sources.into_iter().map(|(_, source)| source).collect(),
                )
            })
            .collect();

        MountPlan {
            tree: self.tree,
            overlay_ops,
            overlay_files,
            overlay_module_ids: self.overlay_module_ids.into_iter().collect(),
            magic_module_ids: self.magic_module_ids.into_iter().collect(),
            vfs_module_ids: self.vfs_module_ids.into_iter().collect(),
        }
    }
}

struct EntryDecision<'a> {
    entry: &'a ModuleEntry,
    mode: Mode,
}

#[allow(clippy::too_many_lines)]
fn process_module(
    module: &ModuleRecord,
    rules: &ModuleRulesView,
    promoted: &BTreeSet<String>,
    builder: &mut PlanBuilder,
) -> Result<()> {
    let decisions: Vec<EntryDecision> = module
        .entries
        .iter()
        .filter(|entry| {
            !is_promoted_partition_alias(entry, promoted)
                && !is_redundant_partition_self_alias(entry, promoted)
        })
        .map(|entry| EntryDecision {
            entry,
            mode: rules.resolve_mode(&entry.relative),
        })
        .collect();

    // 1. Cross-module conflicts: plain directories may be shared, while files, types and `.replace` must be unique.
    for decision in &decisions {
        let (_, target) = map_target(&decision.entry.relative, promoted);
        builder.tree.insert(
            &target,
            MountSource {
                module_id: module.id.clone(),
                relative: decision.entry.relative.clone(),
                source_path: join_relative(&module.source_path, &decision.entry.relative),
                file_type: decision.entry.file_type,
                replace: decision.entry.replace,
                backend: decision.mode,
            },
        );
        if decision.mode != Mode::Ignore {
            builder.register(
                &target,
                decision.mode,
                &module.id,
                &decision.entry.relative,
                decision.entry.file_type,
                decision.entry.replace,
            )?;
        }
    }

    let overlay_count = decisions
        .iter()
        .filter(|decision| decision.mode == Mode::Overlay)
        .count();
    if overlay_count == 0 {
        collect_magic(module, &decisions, builder);
        collect_vfs(module, &decisions, builder);
        return Ok(());
    }

    // 2. Whole-module overlay: every entry is overlay and the module default is overlay,
    //    or a `system = "overlay"` rule exists.
    let all_overlay = overlay_count == decisions.len();
    let whole_overlay = all_overlay
        && (rules.default_mode == Mode::Overlay || rules.has_whole_system_overlay_rule());

    if whole_overlay {
        add_whole_overlay_layers(module, &decisions, promoted, builder);
        return Ok(());
    }

    // 3. Partial overlay: the directory root becomes a lowerdir. Staging materialises from the shared tree per node,
    //    so magic and ignore descendants cannot leak into it.
    let overlay_rels: BTreeSet<&str> = decisions
        .iter()
        .filter(|decision| decision.mode == Mode::Overlay)
        .map(|decision| decision.entry.relative.as_str())
        .collect();

    let overlay_dir_roots: Vec<&ModuleEntry> = decisions
        .iter()
        .filter(|decision| {
            decision.mode == Mode::Overlay && decision.entry.file_type == NodeFileType::Directory
        })
        .map(|decision| decision.entry)
        .filter(|entry| !has_overlay_ancestor(&entry.relative, &overlay_rels))
        .collect();

    for root in &overlay_dir_roots {
        let (partition, target) = map_target(&root.relative, promoted);
        builder.add_overlay_layer(
            &partition,
            &target,
            &module.id,
            join_relative(&module.source_path, &root.relative),
        );
    }

    // 4. File and symlink level overlay: parent directory target to file source, shallow in the executor.
    for decision in &decisions {
        if decision.mode != Mode::Overlay
            || decision.entry.file_type == NodeFileType::Directory
            || has_overlay_ancestor(&decision.entry.relative, &overlay_rels)
        {
            continue;
        }

        let Some((parent, _)) = decision.entry.relative.rsplit_once('/') else {
            continue;
        };
        let (_, parent_target) = map_target(parent, promoted);
        builder.add_overlay_file(
            &parent_target,
            &module.id,
            join_relative(&module.source_path, &decision.entry.relative),
        );
    }

    collect_magic(module, &decisions, builder);
    collect_vfs(module, &decisions, builder);
    Ok(())
}

/// Preserve the v4.2 planner's split-at-partition-root behavior.  Mounting a
/// regular module as one overlay directly on `/system` is both unnecessarily
/// broad and rejected with EINVAL by kernels seen in the field.  The legacy
/// planner descended through `/system` (and promoted partition roots such as
/// `/vendor`) and queued the first real directory below that root instead.
fn add_whole_overlay_layers(
    module: &ModuleRecord,
    decisions: &[EntryDecision<'_>],
    promoted: &BTreeSet<String>,
    builder: &mut PlanBuilder,
) {
    for decision in decisions {
        let relative = decision.entry.relative.as_str();
        let components = relative.split('/').collect::<Vec<_>>();
        let root_len = partition_root_len(&components, promoted);

        if components.len() > root_len + 1
            || (decision.entry.file_type == NodeFileType::Directory
                && components.len() == root_len + 1)
        {
            let layer_relative = components[..root_len + 1].join("/");
            let (partition, target) = map_target(&layer_relative, promoted);
            builder.add_overlay_layer(
                &partition,
                &target,
                &module.id,
                join_relative(&module.source_path, &layer_relative),
            );
            continue;
        }

        // A file directly under a partition root has no child directory that
        // can serve as an overlay layer.  Keep the existing shallow-overlay
        // path for this uncommon, but valid, module layout.
        if decision.entry.file_type != NodeFileType::Directory && components.len() == root_len + 1 {
            let parent = components[..root_len].join("/");
            let (_, target) = map_target(&parent, promoted);
            builder.add_overlay_file(
                &target,
                &module.id,
                join_relative(&module.source_path, relative),
            );
        }
    }
}

fn partition_root_len(components: &[&str], promoted: &BTreeSet<String>) -> usize {
    if components.first() == Some(&"system")
        && components
            .get(1)
            .is_some_and(|partition| promoted.contains(*partition))
    {
        2
    } else {
        1
    }
}

fn collect_magic(
    module: &ModuleRecord,
    decisions: &[EntryDecision<'_>],
    builder: &mut PlanBuilder,
) {
    if !decisions
        .iter()
        .any(|decision| decision.mode == Mode::Magic)
    {
        return;
    }

    builder.magic_module_ids.insert(module.id.clone());
}

fn collect_vfs(module: &ModuleRecord, decisions: &[EntryDecision<'_>], builder: &mut PlanBuilder) {
    if !decisions.iter().any(|decision| decision.mode == Mode::Vfs) {
        return;
    }

    builder.vfs_module_ids.insert(module.id.clone());
}

/// A Magic `.replace` replaces the whole target directory after the Overlay phase, so it
/// cannot contain Overlay descendants that already mounted. An Overlay `.replace` may still receive Magic children.
fn ensure_replace_backend_consistency(node: &MountNode, target: &str) -> Result<()> {
    let current_target = if node.name.is_empty() {
        target.to_owned()
    } else if target.is_empty() {
        format!("/{}", node.name)
    } else {
        format!("{target}/{}", node.name)
    };

    if let Some(replace_source) = node
        .sources
        .iter()
        .find(|source| source.backend == Mode::Magic && source.replace)
        && let Some(overlay_source) = first_descendant_source(node, Mode::Overlay)
    {
        return Err(Error::PlanConflict {
            target: current_target,
            first_backend: "magic".to_owned(),
            first_source: format!(
                "{}:{} (.replace)",
                replace_source.module_id, replace_source.relative
            ),
            second_backend: "overlay".to_owned(),
            second_source: format!("{}:{}", overlay_source.module_id, overlay_source.relative),
        });
    }

    for child in node.children.values() {
        ensure_replace_backend_consistency(child, &current_target)?;
    }
    Ok(())
}

/// VFS rules act on real directories, and any ancestor occupied as a directory by Overlay
/// or Magic shadows injection into its descendants, so it must be an explicit plan-time error.
///
/// The check is conservative: if an ancestor node has an Overlay/Magic directory source,
/// including `.replace`, any Vfs source beneath it counts as shadowed.
fn ensure_vfs_not_shadowed(
    node: &MountNode,
    target: &str,
    ancestor_mount: Option<(Mode, &MountSource)>,
) -> Result<()> {
    let current_target = if node.name.is_empty() {
        target.to_owned()
    } else if target.is_empty() {
        format!("/{}", node.name)
    } else {
        format!("{target}/{}", node.name)
    };

    if let (Some((mode, source)), Some(vfs_source)) = (
        ancestor_mount,
        node.sources
            .iter()
            .find(|source| source.backend == Mode::Vfs),
    ) {
        return Err(Error::PlanConflict {
            target: current_target,
            first_backend: mode.as_str().to_owned(),
            first_source: format!("{}:{}", source.module_id, source.relative),
            second_backend: Mode::Vfs.as_str().to_owned(),
            second_source: format!("{}:{}", vfs_source.module_id, vfs_source.relative),
        });
    }

    let self_mount = node
        .sources
        .iter()
        .find(|source| {
            matches!(source.backend, Mode::Overlay | Mode::Magic)
                && (source.file_type == NodeFileType::Directory || source.replace)
        })
        .map(|source| (source.backend, source));

    let child_mount = self_mount.or(ancestor_mount);
    for child in node.children.values() {
        ensure_vfs_not_shadowed(child, &current_target, child_mount)?;
    }
    Ok(())
}

/// A VFS `.replace` directory replaces its whole subtree as Magisk does: the directory stays
/// visible, real entries are hidden and only injected children show. The subtree must therefore
/// hold no overlay or magic sources, whose mounts the opaque rule would hide.
fn ensure_vfs_opaque_exclusive(
    node: &MountNode,
    target: &str,
    opaque_owner: Option<&MountSource>,
) -> Result<()> {
    let current_target = if node.name.is_empty() {
        target.to_owned()
    } else if target.is_empty() {
        format!("/{}", node.name)
    } else {
        format!("{target}/{}", node.name)
    };

    let owner = node
        .sources
        .iter()
        .find(|source| source.backend == Mode::Vfs && source.replace)
        .or(opaque_owner);

    if let Some(owner) = owner
        && let Some(other) = node
            .sources
            .iter()
            .find(|source| source.backend != Mode::Vfs)
    {
        return Err(Error::PlanConflict {
            target: current_target,
            first_backend: Mode::Vfs.as_str().to_owned(),
            first_source: format!("{}:{} (.replace)", owner.module_id, owner.relative),
            second_backend: other.backend.as_str().to_owned(),
            second_source: format!("{}:{}", other.module_id, other.relative),
        });
    }

    for child in node.children.values() {
        ensure_vfs_opaque_exclusive(child, &current_target, owner)?;
    }
    Ok(())
}

fn first_descendant_source(node: &MountNode, backend: Mode) -> Option<&MountSource> {
    for child in node.children.values() {
        if let Some(source) = child.source_for(backend) {
            return Some(source);
        }
        if let Some(source) = first_descendant_source(child, backend) {
            return Some(source);
        }
    }
    None
}

fn normalize_rule_path(key: &str) -> String {
    key.trim().trim_start_matches('/').to_owned()
}

/// Magisk-style modules commonly carry `system/vendor -> ../vendor` (and the
/// equivalent links for other dynamic partitions) next to the real top-level
/// partition subtree. Once that partition is promoted, the link is layout
/// metadata rather than a mount contribution; mapping it onto `/vendor` would
/// turn a real directory into a symlink in the shared tree.
fn is_promoted_partition_alias(entry: &ModuleEntry, promoted: &BTreeSet<String>) -> bool {
    if entry.file_type != NodeFileType::Symlink {
        return false;
    }

    let mut components = entry.relative.split('/');
    components.next() == Some("system")
        && components
            .next()
            .is_some_and(|partition| promoted.contains(partition))
        && components.next().is_none()
}

/// Older installers could accidentally create `product/product`,
/// `vendor/vendor`, and equivalent self-named symlinks when the promoted
/// top-level partition directory already existed.  They are layout artifacts,
/// not mount contributions.  Treating one as a direct partition child would
/// require a shallow overlay over the entire partition and place that root in
/// KernelSU's try-unmount list.
fn is_redundant_partition_self_alias(entry: &ModuleEntry, promoted: &BTreeSet<String>) -> bool {
    if entry.file_type != NodeFileType::Symlink {
        return false;
    }

    let mut components = entry.relative.split('/');
    let Some(partition) = components.next() else {
        return false;
    };

    promoted.contains(partition)
        && components.next() == Some(partition)
        && components.next().is_none()
}

/// Relative path to (partition, target mountpoint).
fn map_target(relative: &str, promoted: &BTreeSet<String>) -> (String, String) {
    let parts: Vec<&str> = relative.split('/').collect();
    if parts.len() >= 2 && parts[0] == "system" && promoted.contains(parts[1]) {
        let partition = parts[1].to_owned();
        let target = format!("/{}", parts[1..].join("/"));
        (partition, target)
    } else if parts
        .first()
        .is_some_and(|partition| promoted.contains(*partition))
    {
        (parts[0].to_owned(), format!("/{relative}"))
    } else {
        ("system".to_owned(), format!("/{relative}"))
    }
}

fn has_overlay_ancestor(relative: &str, overlay: &BTreeSet<&str>) -> bool {
    let mut end = relative.len();
    while let Some(pos) = relative[..end].rfind('/') {
        let parent = &relative[..pos];
        if overlay.contains(parent) {
            return true;
        }
        end = pos;
    }
    false
}

fn join_relative(root: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(root.to_path_buf(), |path, component| path.join(component))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn record(id: &str, entries: &[(&str, bool)]) -> ModuleRecord {
        ModuleRecord {
            id: ModuleId::try_from(id).unwrap(),
            name: id.to_owned(),
            version: "1".to_owned(),
            author: "a".to_owned(),
            description: "d".to_owned(),
            disabled: false,
            skip_mount: false,
            has_mount_files: true,
            source_path: PathBuf::from(format!("/data/adb/modules/{id}")),
            entries: entries
                .iter()
                .map(|(relative, is_dir)| ModuleEntry {
                    relative: (*relative).to_owned(),
                    file_type: if *is_dir {
                        NodeFileType::Directory
                    } else {
                        NodeFileType::RegularFile
                    },
                    replace: false,
                })
                .collect(),
        }
    }

    fn config(default_mode: Mode, rules: BTreeMap<String, crate::config::ModuleRule>) -> Config {
        let rules = rules
            .into_iter()
            .map(|(id, rule)| (ModuleId::try_from(id).unwrap(), rule))
            .collect();
        Config {
            default_mode,
            rules,
            ..Config::default()
        }
    }

    fn no_rules() -> BTreeMap<String, crate::config::ModuleRule> {
        BTreeMap::new()
    }

    fn plan(modules: &[ModuleRecord], config: &Config, promoted: &[&str]) -> MountPlan {
        plan_with_vfs(modules, config, promoted, true)
    }

    fn plan_with_vfs(
        modules: &[ModuleRecord],
        config: &Config,
        promoted: &[&str],
        vfs_available: bool,
    ) -> MountPlan {
        let promoted: BTreeSet<String> = promoted.iter().map(|name| (*name).to_owned()).collect();
        let input = PlanInput {
            modules,
            config,
            promoted_partitions: &promoted,
            vfs_available,
        };
        build_plan(&input).unwrap()
    }

    fn plan_err(modules: &[ModuleRecord], config: &Config) -> Error {
        let promoted = BTreeSet::new();
        let input = PlanInput {
            modules,
            config,
            promoted_partitions: &promoted,
            vfs_available: true,
        };
        build_plan(&input).unwrap_err()
    }

    #[test]
    fn path_rule_beats_module_and_global_default() {
        let mut rules = no_rules();
        rules.insert(
            "hosts".to_owned(),
            crate::config::ModuleRule {
                default_mode: Some(Mode::Magic),
                paths: BTreeMap::from([("system/etc/hosts".to_owned(), Mode::Overlay)]),
            },
        );
        let config = config(Mode::Overlay, rules);
        let module = record(
            "hosts",
            &[
                ("system/etc", true),
                ("system/etc/hosts", false),
                ("system/etc/other", false),
            ],
        );

        let result = plan(&[module], &config, &[]);

        assert_eq!(result.overlay_module_ids, vec!["hosts"]);
        assert_eq!(
            result.overlay_files["/system/etc"],
            vec![PathBuf::from("/data/adb/modules/hosts/system/etc/hosts")]
        );
        assert_eq!(result.magic_module_ids, vec!["hosts"]);
        assert!(
            result
                .tree
                .find("/system/etc/other")
                .unwrap()
                .source_for(Mode::Magic)
                .is_some()
        );
        assert!(
            result
                .tree
                .find("/system/etc/hosts")
                .unwrap()
                .source_for(Mode::Magic)
                .is_none()
        );
    }

    #[test]
    fn partial_overlay_keeps_multiple_files_from_one_module_and_parent() {
        let mut rules = no_rules();
        rules.insert(
            "hosts".to_owned(),
            crate::config::ModuleRule {
                default_mode: Some(Mode::Magic),
                paths: BTreeMap::from([
                    ("system/etc/hosts".to_owned(), Mode::Overlay),
                    ("system/etc/resolv.conf".to_owned(), Mode::Overlay),
                ]),
            },
        );
        let module = record(
            "hosts",
            &[
                ("system/etc", true),
                ("system/etc/hosts", false),
                ("system/etc/resolv.conf", false),
            ],
        );

        let result = plan(&[module], &config(Mode::Magic, rules), &[]);

        assert_eq!(
            result.overlay_files["/system/etc"],
            vec![
                PathBuf::from("/data/adb/modules/hosts/system/etc/hosts"),
                PathBuf::from("/data/adb/modules/hosts/system/etc/resolv.conf"),
            ]
        );
    }

    #[test]
    fn module_blacklist_overrides_global_and_path_rules() {
        let mut rules = no_rules();
        rules.insert(
            "blocked".to_owned(),
            crate::config::ModuleRule {
                default_mode: Some(Mode::Magic),
                paths: BTreeMap::from([("system/etc/blocked".to_owned(), Mode::Overlay)]),
            },
        );
        let mut config = config(Mode::Overlay, rules);
        config
            .module_blacklist
            .insert(ModuleId::try_from("blocked").unwrap());
        let modules = [
            record("blocked", &[("system/etc/blocked", false)]),
            record("allowed", &[("system/etc/allowed", false)]),
        ];

        let result = plan(&modules, &config, &[]);

        assert_eq!(result.overlay_module_ids, vec!["allowed"]);
        assert!(result.magic_module_ids.is_empty());
        assert!(result.tree.find("/system/etc/blocked").is_none());
        assert!(result.tree.find("/system/etc/allowed").is_some());
    }

    #[test]
    fn leading_slash_rule_key_is_normalized() {
        let mut rules = no_rules();
        rules.insert(
            "hosts".to_owned(),
            crate::config::ModuleRule {
                default_mode: Some(Mode::Magic),
                paths: BTreeMap::from([("/system/etc/hosts".to_owned(), Mode::Overlay)]),
            },
        );
        let config = config(Mode::Ignore, rules);
        let module = record("hosts", &[("system/etc/hosts", false)]);

        let result = plan(&[module], &config, &[]);

        assert!(
            result.overlay_files["/system/etc"]
                .contains(&PathBuf::from("/data/adb/modules/hosts/system/etc/hosts"))
        );
    }

    #[test]
    fn whole_overlay_modules_split_below_system_root_and_keep_module_order() {
        let config = config(Mode::Overlay, no_rules());
        let modules = [
            record("a", &[("system/etc/a", false)]),
            record("b", &[("system/bin/b", false)]),
        ];

        let result = plan(&modules, &config, &[]);

        assert_eq!(result.overlay_ops.len(), 2);
        assert_eq!(result.overlay_ops[0].partition, "system");
        assert_eq!(result.overlay_ops[0].target, "/system/bin");
        assert_eq!(
            result.overlay_ops[0].lowerdirs,
            vec![PathBuf::from("/data/adb/modules/b/system/bin")]
        );
        assert_eq!(result.overlay_ops[1].target, "/system/etc");
        assert_eq!(
            result.overlay_ops[1].lowerdirs,
            vec![PathBuf::from("/data/adb/modules/a/system/etc")]
        );
        assert_eq!(result.overlay_module_ids, vec!["a", "b"]);
        assert!(result.magic_module_ids.is_empty());
    }

    #[test]
    fn whole_overlay_modules_split_below_promoted_partition_root() {
        let config = config(Mode::Overlay, no_rules());
        let module = record(
            "gpu",
            &[
                ("system/vendor", true),
                ("system/vendor/etc", true),
                ("system/vendor/etc/gpu.xml", false),
                ("system/vendor/lib64", true),
                ("system/vendor/lib64/gpu.so", false),
            ],
        );

        let result = plan(&[module], &config, &["vendor"]);

        assert_eq!(result.overlay_ops.len(), 2);
        assert_eq!(result.overlay_ops[0].partition, "vendor");
        assert_eq!(result.overlay_ops[0].target, "/vendor/etc");
        assert_eq!(result.overlay_ops[1].target, "/vendor/lib64");
        assert!(
            result
                .overlay_ops
                .iter()
                .all(|operation| operation.target != "/vendor")
        );
    }

    #[test]
    fn redundant_partition_self_aliases_do_not_mount_partition_roots() {
        let config = config(Mode::Overlay, no_rules());
        let mut module = record(
            "collection",
            &[
                ("product/overlay", true),
                ("product/overlay/product.apk", false),
                ("product/product", false),
                ("vendor/overlay", true),
                ("vendor/overlay/vendor.apk", false),
                ("vendor/vendor", false),
            ],
        );
        module.entries[2].file_type = NodeFileType::Symlink;
        module.entries[5].file_type = NodeFileType::Symlink;

        let result = plan(&[module], &config, &["product", "vendor"]);

        assert_eq!(
            result
                .overlay_ops
                .iter()
                .map(|operation| operation.target.as_str())
                .collect::<Vec<_>>(),
            vec!["/product/overlay", "/vendor/overlay"]
        );
        assert!(!result.overlay_files.contains_key("/product"));
        assert!(!result.overlay_files.contains_key("/vendor"));
        assert!(result.tree.find("/product/product").is_none());
        assert!(result.tree.find("/vendor/vendor").is_none());
    }

    #[test]
    fn top_level_vendor_module_targets_vendor_partition() {
        let module = record(
            "nfc",
            &[("vendor/etc", true), ("vendor/etc/libnfc-nci.conf", false)],
        );
        let config = config(Mode::Overlay, no_rules());

        let result = plan(&[module], &config, &["vendor"]);

        assert_eq!(result.overlay_module_ids, vec!["nfc".to_owned()]);
        assert_eq!(result.overlay_ops.len(), 1);
        assert_eq!(result.overlay_ops[0].partition, "vendor");
        assert_eq!(result.overlay_ops[0].target, "/vendor/etc");
        assert_eq!(
            result.overlay_ops[0].lowerdirs,
            vec![PathBuf::from("/data/adb/modules/nfc/vendor/etc")]
        );
    }

    #[test]
    fn whole_overlay_direct_partition_file_uses_shallow_layer() {
        let config = config(Mode::Overlay, no_rules());
        let module = record("props", &[("system/build.prop", false)]);

        let result = plan(&[module], &config, &[]);

        assert!(result.overlay_ops.is_empty());
        assert_eq!(
            result.overlay_files["/system"],
            vec![PathBuf::from("/data/adb/modules/props/system/build.prop")]
        );
        assert_eq!(result.overlay_module_ids, vec!["props"]);
    }

    #[test]
    fn directory_rule_overlay_uses_directory_as_lowerdir() {
        let mut rules = no_rules();
        rules.insert(
            "m".to_owned(),
            crate::config::ModuleRule {
                default_mode: Some(Mode::Magic),
                paths: BTreeMap::from([("system/etc".to_owned(), Mode::Overlay)]),
            },
        );
        let config = config(Mode::Ignore, rules);
        let module = record(
            "m",
            &[
                ("system/etc", true),
                ("system/etc/hosts", false),
                ("system/etc/sub", true),
                ("system/etc/sub/a", false),
                ("system/bin", true),
                ("system/bin/x", false),
            ],
        );

        let result = plan(&[module], &config, &[]);

        assert_eq!(result.overlay_ops.len(), 1);
        assert_eq!(result.overlay_ops[0].target, "/system/etc");
        assert_eq!(
            result.overlay_ops[0].lowerdirs,
            vec![PathBuf::from("/data/adb/modules/m/system/etc")]
        );
        assert!(result.overlay_files.is_empty());
        // The rest stay magic; the backend choice is stored on the shared tree itself.
        assert!(
            result
                .tree
                .find("/system/bin/x")
                .unwrap()
                .source_for(Mode::Magic)
                .is_some()
        );
        assert!(
            result
                .tree
                .find("/system/etc/hosts")
                .unwrap()
                .source_for(Mode::Magic)
                .is_none()
        );
    }

    #[test]
    fn promoted_partition_rules_target_partition_root() {
        let mut rules = no_rules();
        rules.insert(
            "v".to_owned(),
            crate::config::ModuleRule {
                default_mode: Some(Mode::Magic),
                paths: BTreeMap::from([("system/vendor".to_owned(), Mode::Overlay)]),
            },
        );
        let config = config(Mode::Ignore, rules);
        let module = record(
            "v",
            &[
                ("system/vendor", true),
                ("system/vendor/lib/x.so", false),
                ("system/etc", true),
                ("system/etc/y", false),
            ],
        );

        let result = plan(&[module], &config, &["vendor"]);

        assert_eq!(result.overlay_ops.len(), 1);
        assert_eq!(result.overlay_ops[0].partition, "vendor");
        assert_eq!(result.overlay_ops[0].target, "/vendor");
        assert_eq!(
            result.overlay_ops[0].lowerdirs,
            vec![PathBuf::from("/data/adb/modules/v/system/vendor")]
        );
    }

    #[test]
    fn same_target_different_backends_reports_conflict() {
        let mut rules = no_rules();
        rules.insert(
            "magic_mod".to_owned(),
            crate::config::ModuleRule {
                default_mode: Some(Mode::Magic),
                paths: BTreeMap::new(),
            },
        );
        let config = config(Mode::Overlay, rules);
        let modules = [
            record("overlay_mod", &[("system/etc/hosts", false)]),
            record("magic_mod", &[("system/etc/hosts", false)]),
        ];

        let err = plan_err(&modules, &config);
        assert!(err.to_string().contains("plan conflict"), "{err}");
    }

    #[test]
    fn normal_directories_can_be_shared_by_overlay_and_magic() {
        let mut rules = no_rules();
        rules.insert(
            "magic_mod".to_owned(),
            crate::config::ModuleRule {
                default_mode: Some(Mode::Magic),
                paths: BTreeMap::new(),
            },
        );
        let config = config(Mode::Overlay, rules);
        let modules = [
            record(
                "overlay_mod",
                &[("system/etc", true), ("system/etc/overlay.conf", false)],
            ),
            record(
                "magic_mod",
                &[("system/etc", true), ("system/etc/magic.conf", false)],
            ),
        ];

        let result = plan(&modules, &config, &[]);
        let etc = result.tree.find("/system/etc").unwrap();

        assert_eq!(result.overlay_module_ids, vec!["overlay_mod"]);
        assert_eq!(result.magic_module_ids, vec!["magic_mod"]);
        assert!(etc.source_for(Mode::Overlay).is_some());
        assert!(etc.source_for(Mode::Magic).is_some());
    }

    #[test]
    fn replace_directory_still_conflicts_with_other_backend_at_same_target() {
        let mut rules = no_rules();
        rules.insert(
            "magic_mod".to_owned(),
            crate::config::ModuleRule {
                default_mode: Some(Mode::Magic),
                paths: BTreeMap::new(),
            },
        );
        let overlay = record("overlay_mod", &[("system/etc", true)]);
        let mut magic = record("magic_mod", &[("system/etc", true)]);
        magic.entries[0].replace = true;

        let err = plan_err(&[overlay, magic], &config(Mode::Overlay, rules));
        assert!(err.to_string().contains(".replace"), "{err}");
    }

    #[test]
    fn promoted_layout_aliases_do_not_break_mixed_partition_modules() {
        let mut adreno = record(
            "adreno",
            &[
                ("system/vendor", false),
                ("vendor/etc", true),
                ("vendor/etc/permissions", true),
                ("vendor/etc/permissions/gpu.xml", false),
            ],
        );
        adreno.entries[0].file_type = NodeFileType::Symlink;

        let mut extreme = record(
            "extreme",
            &[
                ("system/vendor", false),
                ("vendor/etc", true),
                ("vendor/etc/perf", true),
                ("vendor/etc/perf/thermal.conf", false),
            ],
        );
        extreme.entries[0].file_type = NodeFileType::Symlink;

        let mut haptics = record(
            "haptics",
            &[
                ("system/odm", false),
                ("odm/lib64", true),
                ("odm/lib64/libhaptic.so", false),
            ],
        );
        haptics.entries[0].file_type = NodeFileType::Symlink;

        let rules = BTreeMap::from([
            (
                "extreme".to_owned(),
                crate::config::ModuleRule {
                    default_mode: Some(Mode::Magic),
                    paths: BTreeMap::new(),
                },
            ),
            (
                "haptics".to_owned(),
                crate::config::ModuleRule {
                    default_mode: Some(Mode::Magic),
                    paths: BTreeMap::new(),
                },
            ),
        ]);

        let result = plan(
            &[adreno, extreme, haptics],
            &config(Mode::Overlay, rules),
            &["vendor", "odm"],
        );

        assert_eq!(result.overlay_module_ids, vec!["adreno"]);
        assert_eq!(result.magic_module_ids, vec!["extreme", "haptics"]);
        assert!(result.tree.find("/vendor").unwrap().sources.is_empty());
        assert!(result.tree.find("/odm").unwrap().sources.is_empty());
        assert_eq!(
            result
                .tree
                .find("/vendor")
                .unwrap()
                .file_type_for(Mode::Magic),
            Some(NodeFileType::Directory)
        );
        assert_eq!(
            result.tree.find("/odm").unwrap().file_type_for(Mode::Magic),
            Some(NodeFileType::Directory)
        );
        let vendor_etc = result.tree.find("/vendor/etc").unwrap();
        assert!(vendor_etc.source_for(Mode::Overlay).is_some());
        assert!(vendor_etc.source_for(Mode::Magic).is_some());
    }

    #[test]
    fn module_overlay_with_magic_path_rule_shares_filtered_directory_tree() {
        let mut rules = no_rules();
        rules.insert(
            "m".to_owned(),
            crate::config::ModuleRule {
                default_mode: None,
                paths: BTreeMap::from([("system/etc/hosts".to_owned(), Mode::Magic)]),
            },
        );
        let config = config(Mode::Overlay, rules);
        let module = record("m", &[("system/etc", true), ("system/etc/hosts", false)]);

        let result = plan(&[module], &config, &[]);
        assert_eq!(result.overlay_ops[0].target, "/system/etc");
        assert!(
            result
                .tree
                .find("/system/etc")
                .unwrap()
                .has_backend(Mode::Overlay)
        );
        assert!(
            result
                .tree
                .find("/system/etc/hosts")
                .unwrap()
                .has_backend(Mode::Magic)
        );
    }

    #[test]
    fn overlay_directory_can_cover_a_magic_descendant_without_staging_it() {
        let mut rules = no_rules();
        rules.insert(
            "m".to_owned(),
            crate::config::ModuleRule {
                default_mode: Some(Mode::Magic),
                paths: BTreeMap::from([
                    ("system/etc".to_owned(), Mode::Overlay),
                    ("system/etc/hosts".to_owned(), Mode::Magic),
                ]),
            },
        );
        let config = config(Mode::Ignore, rules);
        let module = record("m", &[("system/etc", true), ("system/etc/hosts", false)]);

        let result = plan(&[module], &config, &[]);
        assert_eq!(result.overlay_ops[0].target, "/system/etc");
        assert_eq!(result.magic_module_ids, vec!["m"]);
        assert_eq!(
            result
                .tree
                .find("/system/etc/hosts")
                .unwrap()
                .file_type_for(Mode::Magic),
            Some(NodeFileType::RegularFile)
        );
    }

    #[test]
    fn whole_magic_module_is_represented_directly_in_shared_tree() {
        let config = config(Mode::Magic, no_rules());
        let module = record("m", &[("system/etc/hosts", false)]);

        let result = plan(&[module], &config, &[]);

        assert_eq!(result.magic_module_ids, vec!["m"]);
        assert!(result.tree.has_backend(Mode::Magic));
        assert!(result.overlay_ops.is_empty());
    }

    #[test]
    fn rebuilding_a_module_after_backend_switch_drops_the_old_backend() {
        let module = record(
            "switchable",
            &[("system/etc", true), ("system/etc/hosts", false)],
        );

        let overlay_plan = plan(
            std::slice::from_ref(&module),
            &config(Mode::Overlay, no_rules()),
            &[],
        );
        assert_eq!(overlay_plan.overlay_module_ids, vec!["switchable"]);
        assert!(overlay_plan.magic_module_ids.is_empty());
        assert!(overlay_plan.tree.has_backend(Mode::Overlay));
        assert!(!overlay_plan.tree.has_backend(Mode::Magic));

        let magic_plan = plan(&[module], &config(Mode::Magic, no_rules()), &[]);
        assert!(magic_plan.overlay_module_ids.is_empty());
        assert_eq!(magic_plan.magic_module_ids, vec!["switchable"]);
        assert!(!magic_plan.tree.has_backend(Mode::Overlay));
        assert!(magic_plan.tree.has_backend(Mode::Magic));
    }

    #[test]
    fn ignore_rule_removes_magic_backend_from_shared_tree_node() {
        let mut rules = no_rules();
        rules.insert(
            "m".to_owned(),
            crate::config::ModuleRule {
                default_mode: Some(Mode::Magic),
                paths: BTreeMap::from([("system/etc/skip".to_owned(), Mode::Ignore)]),
            },
        );
        let config = config(Mode::Ignore, rules);
        let module = record(
            "m",
            &[("system/etc/keep", false), ("system/etc/skip", false)],
        );

        let result = plan(&[module], &config, &[]);

        assert!(
            result
                .tree
                .find("/system/etc/keep")
                .unwrap()
                .source_for(Mode::Magic)
                .is_some()
        );
        assert!(
            result
                .tree
                .find("/system/etc/skip")
                .unwrap()
                .source_for(Mode::Magic)
                .is_none()
        );
    }

    #[test]
    fn shared_tree_keeps_node_type_replace_and_backend_decision() {
        let config = config(Mode::Overlay, no_rules());
        let mut module = record("m", &[("system/etc", true), ("system/etc/link", false)]);
        module.entries[0].replace = true;
        module.entries[1].file_type = NodeFileType::Symlink;

        let result = plan(&[module], &config, &[]);
        let etc = result.tree.find("/system/etc").unwrap();
        let link = result.tree.find("/system/etc/link").unwrap();

        assert!(etc.replace_for(Mode::Overlay));
        assert_eq!(
            link.file_type_for(Mode::Overlay),
            Some(NodeFileType::Symlink)
        );
        assert!(!result.tree.has_backend(Mode::Magic));
    }

    #[test]
    fn replace_directory_rejects_descendant_from_other_backend() {
        let mut rules = no_rules();
        rules.insert(
            "m".to_owned(),
            crate::config::ModuleRule {
                default_mode: Some(Mode::Magic),
                paths: BTreeMap::from([("system/etc/link".to_owned(), Mode::Overlay)]),
            },
        );
        let mut module = record("m", &[("system/etc", true), ("system/etc/link", false)]);
        module.entries[0].replace = true;

        let err = plan_err(&[module], &config(Mode::Ignore, rules));
        assert!(err.to_string().contains(".replace"), "{err}");
    }

    #[test]
    fn overlay_replace_directory_allows_magic_descendant_after_overlay_phase() {
        let mut rules = no_rules();
        rules.insert(
            "m".to_owned(),
            crate::config::ModuleRule {
                default_mode: Some(Mode::Overlay),
                paths: BTreeMap::from([("system/etc/link".to_owned(), Mode::Magic)]),
            },
        );
        let mut module = record("m", &[("system/etc", true), ("system/etc/link", false)]);
        module.entries[0].replace = true;

        let result = plan(&[module], &config(Mode::Ignore, rules), &[]);
        assert!(
            result
                .tree
                .find("/system/etc")
                .unwrap()
                .replace_for(Mode::Overlay)
        );
        assert!(
            result
                .tree
                .find("/system/etc/link")
                .unwrap()
                .has_backend(Mode::Magic)
        );
    }

    #[test]
    fn magic_replace_rejects_overlay_descendant_from_another_module() {
        let mut rules = no_rules();
        rules.insert(
            "magic".to_owned(),
            crate::config::ModuleRule {
                default_mode: Some(Mode::Magic),
                paths: BTreeMap::new(),
            },
        );
        let mut magic = record("magic", &[("system/etc", true)]);
        magic.entries[0].replace = true;
        let overlay = record("overlay", &[("system/etc/child", false)]);

        let err = plan_err(&[magic, overlay], &config(Mode::Overlay, rules));
        assert!(err.to_string().contains(".replace"), "{err}");
    }

    #[test]
    fn disabled_and_skip_mount_modules_are_excluded() {
        let config = config(Mode::Overlay, no_rules());
        let mut disabled = record("off", &[("system/etc/a", false)]);
        disabled.disabled = true;
        let mut skipped = record("skip", &[("system/etc/b", false)]);
        skipped.skip_mount = true;

        let result = plan(&[disabled, skipped], &config, &[]);

        assert!(result.overlay_ops.is_empty());
        assert!(result.overlay_module_ids.is_empty());
    }

    #[test]
    fn map_target_splits_promoted_partitions() {
        let promoted: BTreeSet<String> = ["vendor", "product"]
            .into_iter()
            .map(str::to_owned)
            .collect();

        assert_eq!(
            map_target("system/etc/hosts", &promoted),
            ("system".to_owned(), "/system/etc/hosts".to_owned())
        );
        assert_eq!(
            map_target("system/vendor/lib/x.so", &promoted),
            ("vendor".to_owned(), "/vendor/lib/x.so".to_owned())
        );
        assert_eq!(
            map_target("system/product/app", &promoted),
            ("product".to_owned(), "/product/app".to_owned())
        );
        assert_eq!(
            map_target("vendor/lib/x.so", &promoted),
            ("vendor".to_owned(), "/vendor/lib/x.so".to_owned())
        );
    }

    #[test]
    fn planning_never_modifies_module_source_fixture() {
        use std::fs;

        let root =
            std::env::temp_dir().join(format!("hybrid-mount-plan-fixture-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let module = root.join("m");
        fs::create_dir_all(module.join("system/etc")).unwrap();
        fs::write(
            module.join("module.prop"),
            "id=m\nname=M\nversion=1\nauthor=A\ndescription=D\n",
        )
        .unwrap();
        let hosts = module.join("system/etc/hosts");
        fs::write(&hosts, "127.0.0.1 localhost").unwrap();

        let scanned = crate::scanner::list_modules(&root, &[]).unwrap();
        let config = config(Mode::Overlay, no_rules());
        let result = plan(&scanned, &config, &[]);

        assert_eq!(
            result.overlay_ops[0].lowerdirs,
            vec![module.join("system/etc")]
        );
        assert_eq!(fs::read_to_string(&hosts).unwrap(), "127.0.0.1 localhost");
        assert_eq!(scanned, crate::scanner::list_modules(&root, &[]).unwrap());

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn vfs_module_is_recorded_in_plan() {
        let module = record("vfs_mod", &[("system/etc/hosts", false)]);
        let result = plan(&[module], &config(Mode::Vfs, no_rules()), &[]);
        assert_eq!(
            result.vfs_module_ids,
            vec![ModuleId::try_from("vfs_mod").unwrap()]
        );
        assert!(result.overlay_module_ids.is_empty());
        assert!(result.magic_module_ids.is_empty());
    }

    /// A kernel without the module has nothing to send rules to: a global `vfs` default must
    /// degrade to `ignore` rather than plan work the executor cannot run.
    #[test]
    fn vfs_default_degrades_to_ignore_without_a_kernel_backend() {
        let module = record("vfs_mod", &[("system/etc/hosts", false)]);
        let result = plan_with_vfs(&[module], &config(Mode::Vfs, no_rules()), &[], false);

        assert!(result.vfs_module_ids.is_empty());
        assert!(result.overlay_module_ids.is_empty());
        assert!(result.magic_module_ids.is_empty());
    }

    #[test]
    fn vfs_path_rule_degrades_to_ignore_without_a_kernel_backend() {
        let mut rules = no_rules();
        rules.insert(
            "vfs_mod".to_owned(),
            crate::config::ModuleRule {
                default_mode: Some(Mode::Magic),
                paths: BTreeMap::from([("system/etc/hosts".to_owned(), Mode::Vfs)]),
            },
        );
        let module = record("vfs_mod", &[("system/etc/hosts", false)]);
        let result = plan_with_vfs(&[module], &config(Mode::Overlay, rules), &[], false);

        assert!(result.vfs_module_ids.is_empty());
        assert!(result.magic_module_ids.is_empty());
    }

    /// The capability must gate VFS only: a magic plan stays intact on the same device.
    #[test]
    fn missing_vfs_backend_keeps_other_backends() {
        let module = record("magic_mod", &[("system/etc/hosts", false)]);
        let result = plan_with_vfs(&[module], &config(Mode::Magic, no_rules()), &[], false);

        assert_eq!(
            result.magic_module_ids,
            vec![ModuleId::try_from("magic_mod").unwrap()]
        );
        assert!(result.vfs_module_ids.is_empty());
    }

    #[test]
    fn vfs_file_under_overlay_directory_is_rejected() {
        let mut rules = no_rules();
        rules.insert(
            "alpha".to_owned(),
            crate::config::ModuleRule {
                default_mode: Some(Mode::Overlay),
                paths: BTreeMap::new(),
            },
        );
        rules.insert(
            "beta".to_owned(),
            crate::config::ModuleRule {
                default_mode: Some(Mode::Vfs),
                paths: BTreeMap::new(),
            },
        );
        let alpha = record("alpha", &[("system/etc", true)]);
        let beta = record("beta", &[("system/etc/hosts", false)]);
        let err = plan_err(&[alpha, beta], &config(Mode::Magic, rules));
        let Error::PlanConflict {
            target,
            first_source,
            second_source,
            ..
        } = err
        else {
            panic!("unexpected: {err}");
        };
        assert!(
            first_source.contains("alpha:"),
            "first_source should name the shadowing module, got: {first_source}"
        );
        assert!(
            second_source.contains("beta:"),
            "second_source should name the shadowed module, got: {second_source}"
        );
        assert_eq!(target, "/system/etc/hosts");
    }

    #[test]
    fn vfs_file_without_mounted_ancestor_is_allowed() {
        let mut rules = no_rules();
        rules.insert(
            "alpha".to_owned(),
            crate::config::ModuleRule {
                default_mode: Some(Mode::Overlay),
                paths: BTreeMap::new(),
            },
        );
        rules.insert(
            "beta".to_owned(),
            crate::config::ModuleRule {
                default_mode: Some(Mode::Vfs),
                paths: BTreeMap::new(),
            },
        );
        let alpha = record("alpha", &[("system/etc/other", true)]);
        let beta = record("beta", &[("system/etc/hosts", false)]);
        let result = plan(&[alpha, beta], &config(Mode::Magic, rules), &[]);
        assert_eq!(result.vfs_module_ids.len(), 1);
    }

    #[test]
    fn vfs_replace_directory_is_allowed() {
        let mut module = record("vfs_replace", &[("system/etc", true)]);
        module.entries[0].replace = true;

        let planned = plan(&[module], &config(Mode::Vfs, no_rules()), &[]);
        assert_eq!(planned.vfs_module_ids.len(), 1);
    }

    #[test]
    fn vfs_replace_subtree_rejects_other_backends() {
        let rules = BTreeMap::from([(
            "vfs_replace".to_owned(),
            crate::config::ModuleRule {
                default_mode: Some(Mode::Vfs),
                paths: BTreeMap::from([("system/etc/hosts".to_owned(), Mode::Overlay)]),
            },
        )]);
        let mut module = record(
            "vfs_replace",
            &[("system/etc", true), ("system/etc/hosts", false)],
        );
        module.entries[0].replace = true;

        let err = plan_err(&[module], &config(Mode::Vfs, rules));
        assert!(
            matches!(err, Error::PlanConflict { .. }),
            "unexpected: {err}"
        );
    }
}
