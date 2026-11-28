use crate::errors::{Error, Result};

#[cfg(any(target_os = "linux", target_os = "android"))]
pub(super) fn handle(args: &[String]) -> Result<()> {
    use crate::framework::runtime::{ModuleAction, apply_module, boot, status_report};
    use crate::mount::model::ModuleId;

    const USAGE: &str =
        "usage: hybrid-mount runtime status|prepare-reboot|load ID|unload ID|reload ID";
    match args {
        [command] if command == "status" => {
            println!("{}", serde_json::to_string(&status_report())?)
        }
        [command] if command == "prepare-reboot" => return boot::cleanup(),
        [command, id] => {
            let action = match command.as_str() {
                "load" => ModuleAction::Load,
                "reload" => ModuleAction::Reload,
                "unload" => ModuleAction::Unload,
                _ => return Err(Error::msg(USAGE)),
            };
            println!(
                "{}",
                apply_module(action, &ModuleId::try_from(id.as_str())?)?
            );
        }
        _ => return Err(Error::msg(USAGE)),
    }
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub(super) fn handle(_args: &[String]) -> Result<()> {
    Err(Error::msg("runtime operations require Linux/Android"))
}
