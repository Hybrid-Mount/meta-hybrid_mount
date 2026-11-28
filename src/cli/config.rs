use std::path::Path;

use crate::defs;
use crate::errors::{Error, Result};
use crate::framework::config::{Config, ConfigPatch, save_patch};

/// Extracts the payload from `["--payload", "<hex>", ...]`.
fn parse_payload_arg(args: &[String]) -> Result<&str> {
    args.windows(2)
        .find_map(|window| (window[0] == "--payload").then_some(window[1].as_str()))
        .ok_or_else(|| Error::msg("missing required --payload argument"))
}

/// hex payload -> UTF-8 JSON text.
fn decode_payload_arg(payload_hex: &str) -> Result<String> {
    let bytes =
        hex::decode(payload_hex).map_err(|err| Error::msg(format!("decode payload hex: {err}")))?;
    String::from_utf8(bytes).map_err(|err| Error::msg(format!("payload is not valid UTF-8: {err}")))
}

/// Parses the payload and merges or persists it to the given path.
fn save_config_payload(path: &Path, payload_hex: &str) -> Result<()> {
    let payload_json = decode_payload_arg(payload_hex)?;
    let patch: ConfigPatch = serde_json::from_str(&payload_json).map_err(|err| {
        Error::msg(format!(
            "parse config payload json for {}: {err}",
            path.display()
        ))
    })?;

    save_patch(path, patch)
}

/// `show-config`: outputs the JSON config.
/// When it is missing, outputs the default with `config_missing: true`; when corrupt or unreadable, outputs the default.
pub(super) fn handle_show_config() -> Result<()> {
    let config = Config::load_or_default(Path::new(defs::CONFIG_PATH))?;
    let tmpfs_xattr_supported = match crate::platform::sys::fs::is_overlay_xattr_supported() {
        Ok(supported) => supported,
        Err(err) => {
            log::warn!("capability probe failed: tmpfs_xattr, error={err}");
            false
        }
    };
    println!("{}", config.to_webui_json(tmpfs_xattr_supported)?);
    Ok(())
}

/// `save-config --payload <hex>`: merges and persists the config, returning `{ok:true}`.
pub(super) fn handle_save_config(args: &[String]) -> Result<()> {
    let payload = parse_payload_arg(args)?;
    save_config_payload(Path::new(defs::CONFIG_PATH), payload)?;
    println!("{}", serde_json::json!({ "ok": true }));
    Ok(())
}

/// `gen-config`: resets to the default config, returning `{ok:true}`.
pub(super) fn handle_gen_config() -> Result<()> {
    Config::write_default(Path::new(defs::CONFIG_PATH))?;
    println!("{}", serde_json::json!({ "ok": true }));
    Ok(())
}

#[cfg(test)]
mod tests;
