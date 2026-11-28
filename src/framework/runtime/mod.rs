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
pub(crate) mod hook_vfs;
#[cfg(any(target_os = "linux", target_os = "android"))]
mod hot;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) use hot::{ModuleAction, apply_module, status_report};

pub(crate) struct ModuleQueryContext {
    pub layout: crate::mount::model::PartitionLayout,
    pub cached: Option<Vec<crate::framework::state::AppModule>>,
    pub boot_owned: std::collections::BTreeSet<String>,
    pub lua_edits: std::collections::BTreeMap<
        crate::mount::model::ModuleId,
        crate::framework::extension::ModuleEdits,
    >,
}

/// Read applied layout, diagnostics and ownership from one boot-scoped ledger snapshot.
pub(crate) fn module_query_context(
    config: &crate::framework::config::Config,
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
                lua_edits: saved.lua_edits,
            });
        }
        crate::platform::namespace::enter_init_namespace()?;
        Ok(ModuleQueryContext {
            layout: crate::framework::scan::partitions::detect(config)?,
            cached: None,
            boot_owned,
            lua_edits: Default::default(),
        })
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        let _ = config;
        Ok(ModuleQueryContext {
            layout: Default::default(),
            cached: None,
            boot_owned: Default::default(),
            lua_edits: Default::default(),
        })
    }
}
