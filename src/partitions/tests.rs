use super::*;

fn evidence(path: &str, ino: u64, mount: i32) -> PathEvidence {
    PathEvidence {
        canonical: path.into(),
        dev: 7,
        ino,
        visible_mount_id: mount,
        is_dir: true,
    }
}

fn mount(point: &str, root: &str, id: i32) -> MountEntry {
    MountEntry {
        mount_point: point.into(),
        root: root.into(),
        parent_id: 1,
        mnt_id: id,
        majmin: "0:7".into(),
        fs_type: "ext4".into(),
        mount_source: None,
        shared: None,
        master: None,
    }
}

fn snapshot() -> LayoutSnapshot {
    LayoutSnapshot {
        mounts: vec![mount("/", "/", 1)],
        paths: BTreeMap::from([(PathBuf::from("/system"), evidence("/system", 2, 1))]),
        path_errors: BTreeMap::new(),
    }
}

fn layout(s: &LayoutSnapshot) -> PartitionLayout {
    discover_layout(s, &["vendor".into()], &[])
}

#[test]
fn aliases_in_both_directions_use_the_resolved_tree() {
    for canonical in ["/vendor", "/system/vendor"] {
        let mut s = snapshot();
        s.mounts.push(mount(canonical, "/", 7));
        s.paths.insert("/vendor".into(), evidence(canonical, 42, 7));
        s.paths
            .insert("/system/vendor".into(), evidence(canonical, 42, 7));
        let l = layout(&s);
        for input in ["vendor/etc/demo", "system/vendor/etc/demo"] {
            assert_eq!(
                l.map_target(input).unwrap(),
                ("vendor".into(), PathBuf::from(canonical).join("etc/demo"))
            );
        }
    }
}

#[test]
fn system_subdirectory_accepts_both_module_layouts() {
    let mut s = snapshot();
    s.paths
        .insert("/system/vendor".into(), evidence("/system/vendor", 42, 1));
    let l = layout(&s);
    assert_eq!(
        l.map_target("vendor/etc/demo").unwrap().1,
        PathBuf::from("/system/vendor/etc/demo")
    );
    assert_eq!(
        l.map_target("system/vendor/etc/demo").unwrap().1,
        PathBuf::from("/system/vendor/etc/demo")
    );
}

#[test]
fn top_level_directory_requires_a_visible_exact_mount() {
    let mut s = snapshot();
    s.paths.insert("/vendor".into(), evidence("/vendor", 42, 1));
    assert!(layout(&s).map_target("vendor/etc/demo").is_err());
    s.mounts.push(mount("/vendor", "/vendor", 3));
    s.mounts.push(mount("/vendor", "/vendor", 99));
    s.paths.insert("/vendor".into(), evidence("/vendor", 42, 3));
    assert_eq!(
        layout(&s).map_target("vendor/etc/demo").unwrap().1,
        PathBuf::from("/vendor/etc/demo")
    );
}

#[test]
fn a_file_or_missing_system_root_is_not_a_partition() {
    let mut s = snapshot();
    let mut file = evidence("/vendor", 42, 2);
    file.is_dir = false;
    s.paths.insert("/vendor".into(), file);
    s.mounts.push(mount("/vendor", "/vendor", 2));
    assert!(layout(&s).map_target("vendor/etc/demo").is_err());
    s.paths.remove(Path::new("/system"));
    assert!(layout(&s).map_target("system/etc/demo").is_err());
}

#[test]
fn different_trees_are_rejected_even_on_the_same_device() {
    let mut s = snapshot();
    s.paths.insert("/vendor".into(), evidence("/vendor", 41, 2));
    s.paths
        .insert("/system/vendor".into(), evidence("/system/vendor", 42, 3));
    s.mounts.extend([
        mount("/vendor", "/vendor", 2),
        mount("/system/vendor", "/other", 3),
    ]);
    let l = layout(&s);
    assert!(l.map_target("vendor/etc/demo").is_err());
    assert!(l.map_target("system/vendor/etc/demo").is_err());
    assert_eq!(
        l.map_target("system/etc/hosts").unwrap().1,
        PathBuf::from("/system/etc/hosts")
    );
}

#[test]
fn bind_aliases_require_identical_descendant_mounts() {
    let mut s = snapshot();
    s.paths.insert("/vendor".into(), evidence("/vendor", 42, 2));
    s.paths
        .insert("/system/vendor".into(), evidence("/system/vendor", 42, 3));
    s.mounts.extend([
        mount("/vendor", "/vendor", 2),
        mount("/system/vendor", "/vendor", 3),
    ]);
    assert_eq!(
        layout(&s).map_target("system/vendor/etc/demo").unwrap().1,
        PathBuf::from("/vendor/etc/demo")
    );
    s.mounts.push(mount("/vendor/etc", "/etc", 4));
    s.paths
        .insert("/vendor/etc".into(), evidence("/vendor/etc", 88, 4));
    assert!(layout(&s).map_target("vendor/etc/demo").is_err());
    s.mounts.push(mount("/system/vendor/etc", "/etc", 5));
    s.paths.insert(
        "/system/vendor/etc".into(),
        evidence("/system/vendor/etc", 88, 5),
    );
    assert!(layout(&s).map_target("vendor/etc/demo").is_ok());
}

#[test]
fn bad_aliases_and_uncertain_paths_fail_locally() {
    let mut s = snapshot();
    s.paths
        .insert("/vendor".into(), evidence("/data/wrong", 42, 1));
    assert!(layout(&s).map_target("vendor/etc/demo").is_err());
    s.paths.insert("/vendor".into(), evidence("/vendor", 42, 2));
    s.mounts.push(mount("/vendor", "/vendor", 2));
    s.path_errors
        .insert("/system/vendor".into(), "symbolic link loop".into());
    assert!(layout(&s).map_target("vendor/etc/demo").is_err());
}

#[test]
fn target_mapping_rejects_traversal_and_prefix_confusion() {
    let l = layout(&snapshot());
    assert!(l.map_target("system/../data/demo").is_err());
    assert!(l.map_target("/system/etc/demo").is_err());
    assert_eq!(
        l.map_target("system/vendor_extra/demo").unwrap().1,
        PathBuf::from("/system/vendor_extra/demo")
    );
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[test]
fn captured_root_identity_matches_the_visible_mount() {
    let records = crate::sys::mountinfo::mount_entries().unwrap();
    let s = capture_layout_snapshot(&[], &records).unwrap();
    let root = &s.paths[Path::new("/")];
    assert!(
        s.mounts
            .iter()
            .any(|m| m.mnt_id == root.visible_mount_id && m.mount_point == Path::new("/"))
    );
}
