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

pub(crate) struct ModuleQueryContext {
    pub layout: crate::partitions::PartitionLayout,
    pub cached: Option<Vec<crate::state::AppModule>>,
    pub boot_owned: std::collections::BTreeSet<String>,
}

/// Read applied layout, diagnostics and ownership from one boot-scoped ledger snapshot.
pub(crate) fn module_query_context(
    config: &crate::config::Config,
) -> crate::errors::Result<ModuleQueryContext> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        let saved = device::load()?;
        let boot_owned = ledger::owned_module_ids(&saved);
        if matches!(saved.phase.as_str(), "ready" | "syncing" | "incomplete")
            && let Some(layout) = saved.partition_layout
        {
            return Ok(ModuleQueryContext {
                layout,
                cached: Some(saved.modules),
                boot_owned,
            });
        }
        device::enter_init_namespace()?;
        Ok(ModuleQueryContext {
            layout: crate::partitions::detect(config)?,
            cached: None,
            boot_owned,
        })
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        let _ = config;
        Ok(ModuleQueryContext {
            layout: Default::default(),
            cached: None,
            boot_owned: Default::default(),
        })
    }
}
