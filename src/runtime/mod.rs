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
