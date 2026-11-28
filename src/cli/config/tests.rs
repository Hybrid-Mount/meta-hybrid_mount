use std::fs;
use std::path::PathBuf;

use super::*;
use crate::mount::model::Mode;
use crate::test_support::Fixture;

#[test]
fn payload_hex_roundtrips_through_save() {
    let dir = Fixture::new("payload");
    let path = dir.join("config.toml");

    let json = r#"{"default_mode":"magic","disable_umount":true}"#;
    let payload_hex = hex::encode(json);
    save_config_payload(&path, &payload_hex).unwrap();

    let saved = Config::load(&path).unwrap();
    assert_eq!(saved.default_mode, Mode::Magic);
    assert!(saved.disable_umount);
    assert_eq!(saved.moduledir, PathBuf::from("/data/adb/modules"));
}

#[test]
fn payload_save_rejects_corrupt_config_without_overwriting_it() {
    let dir = Fixture::new("payload-corrupt");
    let path = dir.join("config.toml");
    let corrupt = "default_mode = not-valid";
    fs::write(&path, corrupt).unwrap();
    let payload_hex = hex::encode(r#"{"default_mode":"magic"}"#);

    let err = save_config_payload(&path, &payload_hex).unwrap_err();

    assert!(err.to_string().contains("parse config"), "{err}");
    assert_eq!(fs::read_to_string(path).unwrap(), corrupt);
}

#[test]
fn payload_arg_requires_marker_and_rejects_invalid_hex() {
    assert_eq!(
        parse_payload_arg(&["--payload".to_owned(), "7b7d".to_owned()]).unwrap(),
        "7b7d"
    );
    assert!(parse_payload_arg(&["x".to_owned()]).is_err());
    assert!(decode_payload_arg("zz").is_err());
    assert_eq!(decode_payload_arg("7b7d").unwrap(), "{}");
}
