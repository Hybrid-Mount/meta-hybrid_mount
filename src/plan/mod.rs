// SPDX-License-Identifier: GPL-3.0-only

//! 混合挂载 planner。
//!
//! 规则优先级:路径规则 > 模块 `default_mode` > 全局 `default_mode`。
//! 同一路径只进入一个后端;冲突在启动时显式报错。
//! 输出:
//! - overlay 操作按目标分区聚合(目录规则直接作为 lowerdir,
//!   文件规则交给执行层做 shallow 层,v4.2.0 prepare 语义);
//! - 同一棵带后端标注的节点树直接交给 OverlayFS 与 Magic Mount 执行层。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::config::{Config, Mode};
use crate::errors::{Error, Result};
use crate::mount_tree::{MountNode, MountSource, MountTree, NodeFileType};
use crate::scanner::{ModuleEntry, ModuleRecord};

/// 一个 overlay 挂载操作(结构与 v4.2.0 `OverlayOperation` 对齐)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayOperation {
    pub partition: String,
    pub target: String,
    pub lowerdirs: Vec<PathBuf>,
}

/// 混合挂载计划。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MountPlan {
    /// scanner 与 planner 共同生成、由两个执行后端共同消费的唯一节点树。
    pub tree: MountTree,
    /// 目录级 overlay:目标挂载点 -> 有序 lowerdirs(按模块 id 排序)。
    pub overlay_ops: Vec<OverlayOperation>,
    /// 文件级 overlay 规则:父目录目标 -> 文件源(执行层做 shallow 层)。
    pub overlay_files: BTreeMap<String, Vec<PathBuf>>,
    pub overlay_module_ids: Vec<String>,
    pub magic_module_ids: Vec<String>,
}

pub struct PlanInput<'a> {
    pub modules: &'a [ModuleRecord],
    pub config: &'a Config,
    /// 执行层按内建分区提升规则探测出的提升分区。
    pub promoted_partitions: &'a BTreeSet<String>,
}

/// 构建挂载计划;任何同一路径跨后端冲突都会返回错误。
pub fn build_plan(input: &PlanInput<'_>) -> Result<MountPlan> {
    let mut modules: Vec<&ModuleRecord> = input.modules.iter().collect();
    modules.sort_by(|left, right| left.id.cmp(&right.id));

    let mut builder = PlanBuilder::default();
    for module in modules {
        if !module.mountable() {
            continue;
        }
        let rules = ModuleRulesView::new(&module.id, input.config);
        process_module(module, &rules, input.promoted_partitions, &mut builder)?;
    }

    ensure_replace_backend_consistency(&builder.tree.root, "")?;
    Ok(builder.finish())
}

struct ModuleRulesView {
    default_mode: Mode,
    /// `(normalized_key, mode)`,按 key 长度降序保证最长前缀优先。
    path_rules: Vec<(String, Mode)>,
}

impl ModuleRulesView {
    fn new(module_id: &str, config: &Config) -> Self {
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

        Self {
            default_mode,
            path_rules,
        }
    }

    fn resolve_mode(&self, relative: &str) -> Mode {
        self.path_rules
            .iter()
            .find(|(key, _)| {
                let prefix = format!("{key}/");
                relative == key.as_str() || relative.starts_with(&prefix)
            })
            .map(|(_, mode)| *mode)
            .unwrap_or(self.default_mode)
    }

    /// 是否存在把整个 `system` 判为 overlay 的路径规则。
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
    overlay_by_target: BTreeMap<String, (String, BTreeMap<String, PathBuf>)>,
    /// 文件规则:父目录 target -> (module id -> file source)
    overlay_files_by_target: BTreeMap<String, BTreeMap<String, PathBuf>>,
    overlay_module_ids: BTreeSet<String>,
    magic_module_ids: BTreeSet<String>,
    /// 跨模块分配表:target -> (backend, 来源描述),用于冲突检测。
    assignments: BTreeMap<String, (Mode, String)>,
}

impl PlanBuilder {
    fn register(
        &mut self,
        target: &str,
        mode: Mode,
        module_id: &str,
        relative: &str,
    ) -> Result<()> {
        let source = format!("{module_id}:{relative}");
        match self.assignments.entry(target.to_owned()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert((mode, source));
            }
            std::collections::btree_map::Entry::Occupied(entry) => {
                let (existing_mode, existing_source) = entry.get();
                if *existing_mode != mode {
                    return Err(Error::PlanConflict {
                        target: target.to_owned(),
                        first_backend: mode_name(*existing_mode).to_owned(),
                        first_source: existing_source.clone(),
                        second_backend: mode_name(mode).to_owned(),
                        second_source: source,
                    });
                }
            }
        }
        Ok(())
    }

    fn add_overlay_layer(
        &mut self,
        partition: &str,
        target: &str,
        module_id: &str,
        source: PathBuf,
    ) {
        let (_, layers) = self
            .overlay_by_target
            .entry(target.to_owned())
            .or_insert_with(|| (partition.to_owned(), BTreeMap::new()));
        layers.insert(module_id.to_owned(), source);
        self.overlay_module_ids.insert(module_id.to_owned());
    }

    fn add_overlay_file(&mut self, target: &str, module_id: &str, source: PathBuf) {
        self.overlay_files_by_target
            .entry(target.to_owned())
            .or_default()
            .insert(module_id.to_owned(), source);
        self.overlay_module_ids.insert(module_id.to_owned());
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
            .map(|(target, sources)| (target, sources.into_values().collect()))
            .collect();

        MountPlan {
            tree: self.tree,
            overlay_ops,
            overlay_files,
            overlay_module_ids: self.overlay_module_ids.into_iter().collect(),
            magic_module_ids: self.magic_module_ids.into_iter().collect(),
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
        .map(|entry| EntryDecision {
            entry,
            mode: rules.resolve_mode(&entry.relative),
        })
        .collect();

    // 1. 跨模块冲突:同一目标路径只能出现一个后端。
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
            builder.register(&target, decision.mode, &module.id, &decision.entry.relative)?;
        }
    }

    let overlay_count = decisions
        .iter()
        .filter(|decision| decision.mode == Mode::Overlay)
        .count();
    if overlay_count == 0 {
        collect_magic(module, &decisions, builder);
        return Ok(());
    }

    // 2. 模块整体 overlay:所有条目都是 overlay,且模块默认 overlay
    //    或存在 `system = "overlay"` 规则。
    let all_overlay = overlay_count == decisions.len();
    let whole_overlay = all_overlay
        && (rules.default_mode == Mode::Overlay || rules.has_whole_system_overlay_rule());

    if whole_overlay {
        add_whole_overlay_layers(module, &decisions, promoted, builder);
        return Ok(());
    }

    // 3. 部分 overlay:目录根直接作 lowerdir。staging 从共享树按节点物化，
    //    因此 magic / ignore 后代不会泄漏进该 lowerdir。
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

    // 4. 文件/符号链接级 overlay:父目录目标 -> 文件源(执行层 shallow)。
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

/// Magic `.replace` 在 Overlay 阶段之后替换整个目标目录，因此不能包含已先行
/// 挂载的 Overlay 后代。Overlay `.replace` 则可由后续 Magic 补入选中子节点。
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

/// 相对路径 -> (分区, 目标挂载点)。
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

fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Overlay => "overlay",
        Mode::Magic => "magic",
        Mode::Ignore => "ignore",
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
            id: id.to_owned(),
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
        let promoted: BTreeSet<String> = promoted.iter().map(|name| (*name).to_owned()).collect();
        let input = PlanInput {
            modules,
            config,
            promoted_partitions: &promoted,
        };
        build_plan(&input).unwrap()
    }

    fn plan_err(modules: &[ModuleRecord], config: &Config) -> Error {
        let promoted = BTreeSet::new();
        let input = PlanInput {
            modules,
            config,
            promoted_partitions: &promoted,
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
        // 其余仍是 magic，后端选择直接保存在共享树上。
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

        let scanned = crate::scanner::list_modules(&root, &[]);
        let config = config(Mode::Overlay, no_rules());
        let result = plan(&scanned, &config, &[]);

        assert_eq!(
            result.overlay_ops[0].lowerdirs,
            vec![module.join("system/etc")]
        );
        // 源目录结构与内容不变
        assert_eq!(fs::read_to_string(&hosts).unwrap(), "127.0.0.1 localhost");
        assert_eq!(scanned, crate::scanner::list_modules(&root, &[]));

        fs::remove_dir_all(&root).ok();
    }
}
