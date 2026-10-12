// SPDX-License-Identifier: GPL-3.0-only

//! Hand-rolled CLI argument parsing and command dispatch, with no clap runtime dependency.
//!
//! No arguments runs the full mount pipeline; the other commands return structured data for the WebUI and diagnostics.

use crate::config::{handle_gen_config, handle_save_config, handle_show_config};
use crate::errors::{Error, Result};
use crate::{pipeline, state};

pub fn run(args: &[String]) -> Result<()> {
    run_with_conflict_check(args, crate::conflicts::check)
}

fn run_with_conflict_check(args: &[String], check: impl FnOnce() -> Result<()>) -> Result<()> {
    // Refuse before locks, boot deduplication, mounting or an insmod fallback.
    // Queries and cleanup remain available so a conflict can be diagnosed and removed.
    let first = args.first().map(String::as_str);
    let second = args.get(1).map(String::as_str);
    if matches!(first, None | Some("boot" | "check-conflicts" | "lkm-load"))
        || (first == Some("runtime") && matches!(second, Some("load" | "reload")))
        || (first == Some("vfs") && second == Some("load"))
    {
        check()?;
    }
    match args.first().map(String::as_str) {
        None => pipeline::run_mount_pipeline(),
        Some("boot") => pipeline::run_boot_pipeline(),
        Some("check-conflicts") => Ok(()),
        Some("show-config") => handle_show_config(),
        Some("save-config") => handle_save_config(args),
        Some("gen-config") => handle_gen_config(),
        Some("modules") => state::handle_modules(),
        Some("status") => state::handle_status(),
        Some("version") => {
            println!("{}", version_payload());
            Ok(())
        }
        Some("install-state") => state::handle_install_state(),
        Some("clear-mount-errors") => state::handle_clear_mount_errors(),
        Some("vfs-doctor") => crate::vfs::doctor::handle(),
        Some("runtime") => crate::runtime::handle(&args[1..]),
        Some("vfs") => crate::vfs::cli::handle(&args[1..]),
        #[cfg(any(target_os = "linux", target_os = "android"))]
        Some("lkm-load") => crate::sys::lkm_compat::handle(&args[1..]),
        Some("emulated-soft-reboot") => emulated_soft_reboot(),
        Some(command) => Err(Error::msg(format!("unknown command: {command}"))),
    }
}

fn version_payload() -> String {
    format!("{{ \"version\": \"{}\" }}", env!("CARGO_PKG_VERSION"))
}

fn emulated_soft_reboot() -> Result<()> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        crate::runtime::boot::cleanup()
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        Err(Error::msg(
            "emulated-soft-reboot is only supported on linux/android",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflicts_stop_every_loading_entry_before_dispatch() {
        for command in [
            vec![],
            vec!["boot"],
            vec!["check-conflicts"],
            vec!["lkm-load", "missing.ko"],
            vec!["runtime", "load", "demo"],
            vec!["runtime", "reload", "demo"],
            vec!["vfs", "load"],
        ] {
            let args = command.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
            let result = run_with_conflict_check(&args, || Err(Error::msg("conflict sentinel")));
            assert_eq!(
                result.unwrap_err().to_string(),
                "conflict sentinel",
                "{command:?}"
            );
        }
    }

    #[test]
    fn conflict_does_not_disable_version_or_help() {
        for command in [vec!["version"], vec!["vfs", "help"]] {
            let args = command.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
            run_with_conflict_check(&args, || panic!("query must not check conflicts")).unwrap();
        }
    }

    #[test]
    fn version_command_matches_contract_shape() {
        run(&["version".to_owned()]).unwrap();
        assert_eq!(
            version_payload(),
            format!(r#"{{ "version": "{}" }}"#, env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn unknown_command_is_rejected() {
        let err = run(&["not-a-command".to_owned()]).unwrap_err();
        assert!(err.to_string().contains("unknown command"), "{err}");
    }
}
