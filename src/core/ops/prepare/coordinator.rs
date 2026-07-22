// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

use anyhow::{Context, Result, bail};

use super::{
    module_processor::{
        materialize_module_identity, module_requests_kasumi, module_sync_error, prepare_module,
    },
    plan_builder::{merge_overlay_groups, sorted_ids},
    types::PrepareContext,
};
use crate::{
    core::{
        backend_capabilities::BackendCapabilities,
        inventory::Module,
        ops::plan::{MountPlan, OverlayOperation},
    },
    partitions,
    sys::fs::{PreparedDir, finalize_copied_tree, prune_orphaned_children, remove_path},
    utils,
};

pub fn prepare_mount_plan(
    modules: &[Module],
    target_base: &Path,
    capabilities: &BackendCapabilities,
) -> Result<MountPlan> {
    prepare_mount_plan_with_root(
        modules,
        target_base,
        Path::new("/"),
        capabilities,
        partitions::managed_partition_names(),
    )
}

pub(crate) fn prepare_mount_plan_with_root(
    modules: &[Module],
    target_base: &Path,
    system_root: &Path,
    capabilities: &BackendCapabilities,
    managed_partitions: Vec<String>,
) -> Result<MountPlan> {
    let prepare_started = Instant::now();
    crate::scoped_log!(
        info,
        "prepare",
        "start: modules={}, storage_root={}",
        modules.len(),
        target_base.display()
    );

    if modules.iter().any(module_requests_kasumi) && !capabilities.can_use_kasumi() {
        bail!(
            "Kasumi rules require an available Kasumi backend (status: {})",
            capabilities.kasumi_status()
        );
    }

    fs::create_dir_all(target_base)
        .with_context(|| format!("failed to create storage root {}", target_base.display()))?;
    crate::scoped_log!(
        debug,
        "prepare",
        "storage root created: {}",
        target_base.display()
    );
    prune_orphaned_children(
        target_base,
        modules.iter().map(|module| module.id.as_str()),
        &["lost+found", "hybrid_mount"],
        "prepare",
    )?;

    let module_rank: HashMap<&str, usize> = modules
        .iter()
        .enumerate()
        .map(|(idx, module)| (module.id.as_str(), idx))
        .collect();
    let managed_set = managed_partitions.into_iter().collect::<HashSet<_>>();
    let mut context = PrepareContext::new(managed_set);
    let mut overlay_groups: BTreeMap<PathBuf, (String, Vec<PathBuf>)> = BTreeMap::new();
    let mut magic_ids = HashSet::new();
    #[cfg(feature = "kasumi")]
    let mut kasumi_ids = HashSet::new();

    for module in modules {
        crate::scoped_log!(
            debug,
            "prepare",
            "module process: id={}, source={}",
            module.id,
            module.source_path.display()
        );
        let prepared = PreparedDir::new(target_base, &module.id)
            .map_err(|err| module_sync_error(module, err))?;
        let outcome = prepare_module(
            module,
            prepared.tmp_path(),
            prepared.final_path(),
            system_root,
            &mut context,
        )
        .map_err(|err| module_sync_error(module, err))?;

        let keep_module = outcome.has_mount_content && outcome.plan.has_mount_result();
        if !keep_module {
            crate::scoped_log!(
                debug,
                "prepare",
                "module skip: id={}, reason={}",
                module.id,
                if outcome.has_mount_content {
                    "no_mount_plan"
                } else {
                    "no_mount_content"
                }
            );
            remove_path(prepared.final_path()).with_context(|| {
                format!(
                    "failed to clean stale prepared module {} at {}",
                    module.id,
                    prepared.final_path().display()
                )
            })?;
            continue;
        }

        let has_overlay = !outcome.plan.overlay_groups.is_empty();
        if has_overlay {
            materialize_module_identity(module, prepared.tmp_path(), &mut context)
                .map_err(|err| module_sync_error(module, err))?;
            finalize_copied_tree(&module.id, prepared.tmp_path(), &outcome.opaque_dirs)?;
            prepared
                .commit()
                .map_err(|err| module_sync_error(module, err))?;
        } else {
            remove_path(prepared.final_path()).with_context(|| {
                format!(
                    "failed to clean non-overlay prepared module {} at {}",
                    module.id,
                    prepared.final_path().display()
                )
            })?;
        }

        crate::scoped_log!(
            debug,
            "prepare",
            "module prepared: id={}, overlay={}, magic={}, kasumi={}",
            module.id,
            !outcome.plan.overlay_groups.is_empty(),
            outcome.plan.magic,
            outcome.plan.kasumi
        );

        merge_overlay_groups(&mut overlay_groups, outcome.plan.overlay_groups);
        if outcome.plan.magic {
            magic_ids.insert(module.id.clone());
        }
        #[cfg(feature = "kasumi")]
        if outcome.plan.kasumi {
            kasumi_ids.insert(module.id.clone());
        }
    }

    let mut overlay_module_ids = HashSet::new();
    let mut overlay_ops = Vec::with_capacity(overlay_groups.len());
    for (target_path, (partition_name, mut layers)) in overlay_groups {
        layers.sort_by_cached_key(|path| {
            let module_id = utils::extract_module_id(path).filter(|id| !id.is_empty());
            (
                module_id
                    .as_deref()
                    .and_then(|id| module_rank.get(id))
                    .copied()
                    .unwrap_or(usize::MAX),
                path.clone(),
            )
        });

        for layer in &layers {
            if let Some(module_id) = utils::extract_module_id(layer) {
                overlay_module_ids.insert(module_id);
            }
        }

        crate::scoped_log!(
            info,
            "prepare",
            "overlay op: partition={}, target={}, layers={}",
            partition_name,
            target_path.display(),
            layers.len()
        );

        overlay_ops.push(OverlayOperation {
            partition_name,
            target: target_path.display().to_string(),
            lowerdirs: layers,
        });
    }

    context.metrics.elapsed_ms =
        u64::try_from(prepare_started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let plan = MountPlan {
        prepare_metrics: context.metrics,
        overlay_ops,
        #[cfg(feature = "kasumi")]
        kasumi_add_rules: Vec::new(),
        #[cfg(feature = "kasumi")]
        kasumi_merge_rules: Vec::new(),
        #[cfg(feature = "kasumi")]
        kasumi_hide_rules: Vec::new(),
        overlay_module_ids: sorted_ids(overlay_module_ids),
        magic_module_ids: sorted_ids(magic_ids),
        #[cfg(feature = "kasumi")]
        kasumi_module_ids: sorted_ids(kasumi_ids),
    };

    crate::scoped_log!(
        info,
        "prepare",
        "complete: overlay_ops={}, overlay_modules={}, magic_modules={}, kasumi_modules={}, elapsed_ms={}, directories_scanned={}, entries_scanned={}, copied_entries={}, copied_bytes={}, kasumi_rule_compile=deferred",
        plan.overlay_ops.len(),
        plan.overlay_module_ids.len(),
        plan.magic_module_ids.len(),
        {
            #[cfg(feature = "kasumi")]
            {
                plan.kasumi_module_ids.len()
            }
            #[cfg(not(feature = "kasumi"))]
            {
                0usize
            }
        },
        plan.prepare_metrics.elapsed_ms,
        plan.prepare_metrics.directories_scanned,
        plan.prepare_metrics.entries_scanned,
        plan.prepare_metrics.copied_entries,
        plan.prepare_metrics.copied_bytes,
    );

    Ok(plan)
}
