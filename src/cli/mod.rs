// SPDX-License-Identifier: GPL-3.0-only

//! Hand-rolled CLI argument parsing and command dispatch, with no clap runtime dependency.
//!
//! No arguments runs the full mount pipeline; the other commands return structured data for the WebUI and diagnostics.

mod config;
mod runtime;
mod vfs;

use self::config::{handle_gen_config, handle_save_config, handle_show_config};
use crate::errors::{Error, Result};
use crate::framework::boot::hook::Stage;
use crate::framework::{boot, state};

pub fn run(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        None => boot::run_mount_pipeline(),
        Some("boot") => boot::hook::run(boot_stage(&args[1..])?),
        Some("show-config") => handle_show_config(),
        Some("save-config") => handle_save_config(args),
        Some("gen-config") => handle_gen_config(),
        Some("modules") => print_json(&state::modules()?),
        Some("status") => print_json(&state::status()),
        Some("version") => {
            println!("{}", version_payload());
            Ok(())
        }
        Some("install-state") => print_json(&state::install_state()),
        Some("clear-mount-errors") => {
            println!("{}", clear_errors_payload(state::clear_mount_errors()?));
            Ok(())
        }
        Some("vfs-doctor") => print_json(&crate::mount::vfs::doctor::report()),
        Some("runtime") => runtime::handle(&args[1..]),
        Some("vfs") => vfs::handle(&args[1..]),
        #[cfg(any(target_os = "linux", target_os = "android"))]
        Some("lkm-load") => {
            let (path, parameters) = args[1..]
                .split_first()
                .ok_or_else(|| Error::msg("usage: lkm-load <module.ko> [parameters...]"))?;
            crate::platform::sys::lkm_compat::load(std::path::Path::new(path), parameters)
        }
        Some("emulated-soft-reboot") => emulated_soft_reboot(),
        Some(command) => Err(Error::msg(format!("unknown command: {command}"))),
    }
}

fn boot_stage(args: &[String]) -> Result<Stage> {
    match args {
        [] => Ok(Stage::Manual),
        [stage] if stage == "metamount" => Ok(Stage::Metamount),
        [stage] if stage == "post-mount" => Ok(Stage::PostMount),
        _ => Err(Error::msg("usage: boot [metamount|post-mount]")),
    }
}

fn print_json(value: &impl serde::Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn clear_errors_payload(removed: usize) -> String {
    serde_json::json!({ "ok": true, "removed": removed }).to_string()
}

fn version_payload() -> String {
    format!("{{ \"version\": \"{}\" }}", env!("CARGO_PKG_VERSION"))
}

fn emulated_soft_reboot() -> Result<()> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        crate::framework::runtime::boot::cleanup()
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
    fn unknown_or_extra_stage_arguments_are_rejected() {
        for args in [
            vec!["late".into()],
            vec!["metamount".into(), "extra".into()],
        ] {
            assert!(boot_stage(&args).is_err());
        }
    }

    #[test]
    fn clear_errors_wire_snapshot_is_stable() {
        assert_eq!(clear_errors_payload(3), r#"{"ok":true,"removed":3}"#);
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
