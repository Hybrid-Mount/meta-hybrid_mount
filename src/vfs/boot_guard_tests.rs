// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use crate::test_support::Fixture;

/// An identity that is never the running build's, so markers written with it stay foreign.
fn other_identity() -> GuardIdentity {
    GuardIdentity {
        version: "9.9.9".to_owned(),
        binary: Some("42:7".to_owned()),
        object: None,
    }
}

/// This build's identity, narrowed to a kernel object it does not necessarily ship.
fn identity_for_object(object: Option<&str>) -> GuardIdentity {
    GuardIdentity::for_object(object.map(str::to_owned))
}

fn write_module_guard(dir: &Path, identity: &GuardIdentity) -> PathBuf {
    write_marker(
        dir,
        "lkm_guard",
        &marker_payload(VfsMutationSource::Module, identity),
    )
}

fn write_marker(dir: &Path, name: &str, text: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, text).unwrap();
    path
}

#[test]
fn a_handled_return_clears_the_marker_it_armed() {
    let fixture = Fixture::new("vfs-guard-drop");
    let path = fixture.join("guard");
    let guard = VfsBootGuard::arm_at(&path, VfsMutationSource::Runtime, &other_identity()).unwrap();

    assert!(path.exists());
    drop(guard);
    assert!(!path.exists());
}

#[test]
fn the_marker_payload_identifies_the_build_and_the_mutation() {
    let fixture = Fixture::new("vfs-guard-payload");
    let path = fixture.join("guard");
    let identity = other_identity();
    let guard = VfsBootGuard::arm_at(&path, VfsMutationSource::Cleanup, &identity).unwrap();

    let text = fs::read_to_string(&path).unwrap();
    let record = GuardRecord::parse(&text).unwrap();
    assert_eq!(record.version.as_deref(), Some("9.9.9"));
    assert_eq!(record.binary.as_deref(), Some("42:7"));
    assert_eq!(record.source.as_deref(), Some("cleanup"));
    assert!(record.matches(&identity));
    assert!(text.ends_with('\n'));

    drop(guard);
}

#[test]
fn arming_refuses_an_existing_marker_without_clearing_it() {
    let fixture = Fixture::new("vfs-guard-refuse");
    let path = write_marker(&fixture, "guard", "version=1.0.0\nsource=boot\n");

    assert!(VfsBootGuard::arm_at(&path, VfsMutationSource::Runtime, &other_identity()).is_err());
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "version=1.0.0\nsource=boot\n"
    );
}

#[test]
fn a_marker_of_this_build_is_owned() {
    let fixture = Fixture::new("vfs-guard-own");
    let identity = other_identity();
    let path = fixture.join("guard");
    fs::write(&path, marker_payload(VfsMutationSource::Runtime, &identity)).unwrap();

    match inspect_at(&path, &identity).unwrap() {
        VfsGuardVerdict::Own(record) => {
            assert_eq!(record.source.as_deref(), Some("runtime"));
        }
        other => panic!("a marker written by this build must be owned: {other:?}"),
    }
}

#[test]
fn a_marker_of_another_build_is_foreign() {
    let fixture = Fixture::new("vfs-guard-foreign-version");
    let path = write_marker(
        &fixture,
        "guard",
        "version=1.0.0\nbinary=42:7\nboot=boot-id\nsource=runtime\n",
    );

    match inspect_at(&path, &other_identity()).unwrap() {
        VfsGuardVerdict::Foreign { record } => {
            assert!(describe(record.as_ref()).contains("version=1.0.0"));
        }
        other => panic!("a marker of another build must be foreign: {other:?}"),
    }
}

#[test]
fn a_rebuilt_executable_of_the_same_version_is_foreign() {
    let fixture = Fixture::new("vfs-guard-rebuilt");
    let path = write_marker(
        &fixture,
        "guard",
        "version=9.9.9\nbinary=41:7\nsource=boot\n",
    );

    assert_eq!(
        inspect_at(&path, &other_identity()).unwrap(),
        VfsGuardVerdict::Foreign {
            record: GuardRecord::parse("version=9.9.9\nbinary=41:7\nsource=boot\n")
        }
    );
}

#[test]
fn the_legacy_one_byte_marker_is_foreign_without_identity() {
    let fixture = Fixture::new("vfs-guard-legacy");
    let path = write_marker(&fixture, "guard", "1");

    assert_eq!(
        inspect_at(&path, &other_identity()).unwrap(),
        VfsGuardVerdict::Foreign { record: None }
    );
    assert_eq!(describe(None), "no identity (legacy marker)");
}

#[test]
fn a_missing_marker_is_absent_and_already_clear() {
    let fixture = Fixture::new("vfs-guard-absent");
    let path = fixture.join("missing");

    assert_eq!(
        inspect_at(&path, &other_identity()).unwrap(),
        VfsGuardVerdict::Absent
    );
    assert_eq!(read_marker(&path).unwrap(), None);
    assert!(!clear_at(&path).unwrap());
}

#[test]
fn clearing_removes_a_present_marker() {
    let fixture = Fixture::new("vfs-guard-clear");
    let path = write_marker(&fixture, "guard", "version=1.0.0\n");

    assert!(clear_at(&path).unwrap());
    assert!(!path.exists());
    assert_eq!(read_marker(&path).unwrap(), None);
}

#[test]
fn the_module_payload_records_the_candidate_and_its_object() {
    let fixture = Fixture::new("vfs-lkm-payload");
    let candidate = write_marker(&fixture, "hybridmount.ko", "module");

    let payload = lkm_attempt_payload(&candidate);
    let record = GuardRecord::parse(&payload).unwrap();
    assert_eq!(record.source.as_deref(), Some("module"));
    assert_eq!(record.object, file_identity(&candidate));
    assert!(payload.contains(&format!("lkm={}", candidate.display())));
}

#[test]
fn a_module_guard_for_a_packaged_object_refuses_the_retry() {
    let fixture = Fixture::new("vfs-lkm-own");
    let candidate = write_marker(&fixture, "hybridmount.ko", "module");
    let guard = write_module_guard(
        &fixture,
        &identity_for_object(file_identity(&candidate).as_deref()),
    );

    let error = prepare_lkm_guard_at(&guard, &[candidate]).unwrap_err();
    assert!(error.to_string().contains("vfs guard clear"));
    assert!(guard.exists());
}

#[test]
fn a_module_guard_for_a_replaced_object_is_retired() {
    let fixture = Fixture::new("vfs-lkm-refreshed");
    let candidate = write_marker(&fixture, "hybridmount.ko", "module");
    let guard = write_module_guard(&fixture, &identity_for_object(Some("1:1")));

    prepare_lkm_guard_at(&guard, &[candidate]).unwrap();
    assert!(!guard.exists());
}

#[test]
fn a_module_guard_of_another_build_is_retired() {
    let fixture = Fixture::new("vfs-lkm-foreign");
    let candidate = write_marker(&fixture, "hybridmount.ko", "module");
    let guard = write_marker(
        &fixture,
        "lkm_guard",
        "version=1.0.0\nbinary=42:7\nobject=1:1\nsource=module\n",
    );

    prepare_lkm_guard_at(&guard, &[candidate]).unwrap();
    assert!(!guard.exists());
}

#[test]
fn the_legacy_module_guard_is_retired() {
    let fixture = Fixture::new("vfs-lkm-legacy");
    let candidate = write_marker(&fixture, "hybridmount.ko", "module");
    let guard = write_marker(
        &fixture,
        "lkm_guard",
        "lkm=/data/adb/modules/hybrid_mount/vfs/binaries/x.ko\n",
    );

    prepare_lkm_guard_at(&guard, &[candidate]).unwrap();
    assert!(!guard.exists());
}

#[test]
fn a_module_guard_without_an_object_still_blames_this_build() {
    let fixture = Fixture::new("vfs-lkm-no-object");
    let guard = write_marker(
        &fixture,
        "lkm_guard",
        &marker_payload(VfsMutationSource::Module, &GuardIdentity::current()),
    );

    assert!(prepare_lkm_guard_at(&guard, &[]).is_err());
    assert!(guard.exists());
}

#[test]
fn the_module_guard_report_names_its_verdict() {
    let fixture = Fixture::new("vfs-lkm-report");
    let absent = fixture.join("missing");
    let legacy = write_marker(&fixture, "legacy", "lkm=/x.ko\n");
    let own = write_marker(
        &fixture,
        "own",
        &marker_payload(VfsMutationSource::Module, &GuardIdentity::current()),
    );
    let stale = write_marker(&fixture, "stale", "version=1.0.0\nbinary=42:7\n");

    assert_eq!(lkm_guard_at(&absent).unwrap().verdict, "absent");
    assert_eq!(lkm_guard_at(&legacy).unwrap().verdict, "unattributed");
    assert_eq!(lkm_guard_at(&own).unwrap().verdict, "own");
    assert_eq!(lkm_guard_at(&stale).unwrap().verdict, "stale");
}
