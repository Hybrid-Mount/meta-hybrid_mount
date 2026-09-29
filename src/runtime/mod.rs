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
pub use device::enter_init_namespace;

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
