// SPDX-License-Identifier: GPL-3.0-only

pub mod ledger;
mod lifecycle;
#[cfg(any(target_os = "linux", target_os = "android", test))]
pub mod mounts;
mod policy;
pub mod rules;
mod transaction;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub mod boot;
#[cfg(any(target_os = "linux", target_os = "android"))]
mod device;
#[cfg(any(target_os = "linux", target_os = "android"))]
mod hot;

pub fn handle(args: &[String]) -> crate::errors::Result<()> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        hot::handle(args)
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        let _ = args;
        Err(crate::errors::Error::msg(
            "runtime operations require Linux/Android",
        ))
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub use device::{boot_id, enter_init_namespace};

/// Only a boot-scoped ledger may supply applied layout/diagnostic provenance.
pub(crate) fn module_query_context(
    config: &crate::config::Config,
) -> crate::errors::Result<(
    crate::partitions::PartitionLayout,
    Option<Vec<crate::state::AppModule>>,
)> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        let saved = device::load()?;
        if matches!(saved.phase.as_str(), "ready" | "syncing" | "incomplete")
            && let Some(layout) = saved.partition_layout
        {
            return Ok((layout, Some(saved.modules)));
        }
        device::enter_init_namespace()?;
        Ok((crate::partitions::detect(config)?, None))
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        let _ = config;
        Ok((crate::partitions::PartitionLayout::default(), None))
    }
}

/// Module ids whose mounts this kernel boot still owns, from the boot-scoped runtime ledger.
///
/// `scan.ret` outlives the boot that wrote it, so a module query must not treat its committed
/// `is_mounted` flags as proof that those mounts are still live. An unreadable ledger reports no
/// ownership instead of failing the query.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn boot_owned_module_ids() -> std::collections::BTreeSet<String> {
    device::load()
        .map(|ledger| ledger::owned_module_ids(&ledger))
        .unwrap_or_default()
}

/// See the Linux/Android implementation: platforms without a runtime ledger own no mounts.
#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn boot_owned_module_ids() -> std::collections::BTreeSet<String> {
    std::collections::BTreeSet::new()
}
