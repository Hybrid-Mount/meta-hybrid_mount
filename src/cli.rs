// SPDX-License-Identifier: GPL-3.0-only

//! 手工 CLI 参数解析与命令分派(无 clap 运行时依赖)。
//!
//! 无参数执行完整挂载流水线；其余命令为 WebUI 与诊断工具提供结构化数据。

use std::path::Path;

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::config::Config;
use crate::config::{handle_gen_config, handle_save_config, handle_show_config};
use crate::defs;
use crate::errors::{Error, Result};
use crate::{pipeline, state};

pub fn run(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        None => pipeline::run_mount_pipeline(),
        Some("show-config") => handle_show_config(),
        Some("save-config") => handle_save_config(args),
        Some("gen-config") => handle_gen_config(),
        Some("modules") => state::handle_modules(),
        Some("status") => state::handle_status(),
        Some("version") => {
            println!("{{ \"version\": \"{}\" }}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("install-state") => state::handle_install_state(),
        Some("clear-mount-errors") => state::handle_clear_mount_errors(),
        Some("emulated-soft-reboot") => emulated_soft_reboot(),
        Some(command) => Err(Error::msg(format!("unknown command: {command}"))),
    }
}

fn emulated_soft_reboot() -> Result<()> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        let config = Config::load_or_default(Path::new(defs::CONFIG_PATH));
        crate::utils::ksu::init();
        let source =
            pipeline::effective_mount_source(&config.mountsource, crate::utils::ksu::is_active());
        crate::sys::mount::emulated_soft_reboot(source)
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        let _ = Path::new(defs::CONFIG_PATH);
        Err(Error::msg(
            "emulated-soft-reboot is only supported on linux/android",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_command_matches_contract_shape() {
        run(&["version".to_owned()]).unwrap();
    }

    #[test]
    fn unknown_command_is_rejected() {
        let err = run(&["not-a-command".to_owned()]).unwrap_err();
        assert!(err.to_string().contains("unknown command"), "{err}");
    }
}
