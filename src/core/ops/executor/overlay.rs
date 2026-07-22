// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::path::Path;

use anyhow::Result;

#[cfg(feature = "kasumi")]
use crate::core::kasumi_coordinator::KasumiCoordinator;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::mount::umount_mgr;
use crate::{conf::config, core::ops::plan::OverlayOperation, defs, mount::overlayfs};

#[cfg(feature = "kasumi")]
pub(super) fn mount_overlay(
    op: &OverlayOperation,
    config: &config::Config,
    kasumi: &KasumiCoordinator<'_>,
) -> Result<Vec<String>> {
    mount_overlay_inner(op, config, Some(kasumi))
}

#[cfg(not(feature = "kasumi"))]
pub(super) fn mount_overlay(op: &OverlayOperation, config: &config::Config) -> Result<Vec<String>> {
    mount_overlay_inner(op, config)
}

#[cfg(feature = "kasumi")]
fn mount_overlay_inner(
    op: &OverlayOperation,
    config: &config::Config,
    kasumi: Option<&KasumiCoordinator<'_>>,
) -> Result<Vec<String>> {
    mount_overlay_base(op, config)?;
    if let Some(kasumi) = kasumi {
        kasumi.hide_overlay_xattrs(Path::new(&op.target))?;
    }
    Ok(super::collect_involved_modules(op))
}

#[cfg(not(feature = "kasumi"))]
fn mount_overlay_inner(op: &OverlayOperation, config: &config::Config) -> Result<Vec<String>> {
    mount_overlay_base(op, config)?;
    Ok(super::collect_involved_modules(op))
}

fn mount_overlay_base(op: &OverlayOperation, config: &config::Config) -> Result<()> {
    let involved_modules = super::collect_involved_modules(op);

    crate::scoped_log!(
        debug,
        "executor:overlay",
        "prepare: target={}, partition={}, modules={}",
        op.target,
        op.partition_name,
        if involved_modules.is_empty() {
            "<unknown>".to_string()
        } else {
            involved_modules.join(",")
        }
    );

    let lowerdir_strings: Vec<String> = op
        .lowerdirs
        .iter()
        .map(|p| p.display().to_string())
        .collect();

    let rw_root = Path::new(defs::SYSTEM_RW_DIR);
    let part_rw = rw_root.join(&op.partition_name);
    let upper = part_rw.join("upperdir");
    let work = part_rw.join("workdir");

    let (upper_opt, work_opt) = match (upper.exists(), work.exists()) {
        (true, true) => (Some(upper), Some(work)),
        (false, false) => (None, None),
        _ => anyhow::bail!(
            "overlay upper/work directories are inconsistent for {}",
            op.partition_name
        ),
    };

    overlayfs::overlayfs::mount_overlay(
        &op.target,
        &lowerdir_strings,
        work_opt,
        upper_opt,
        &config.mountsource,
    )?;

    crate::scoped_log!(
        debug,
        "executor:overlay",
        "complete: target={}, mount_source={}",
        op.target,
        config.mountsource
    );

    #[cfg(any(target_os = "linux", target_os = "android"))]
    if !config.disable_umount {
        umount_mgr::send_umountable(&op.target)?;
    }

    Ok(())
}
