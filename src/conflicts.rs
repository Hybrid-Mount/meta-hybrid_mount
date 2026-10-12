// SPDX-License-Identifier: GPL-3.0-only

//! Blocks the susfs4ksu-lkm project, not SUSFS built into a kernel.
//! Do not probe generic SUSFS proc interfaces, symbols or CONFIG_KSU_SUSFS.

use std::{fs, io, path::Path};

use crate::errors::{Error, Result};

const MODULE_NAME: &str = "susfs_guard_lkm";

pub(crate) fn check() -> Result<()> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        check_for_runtime(
            Path::new("/data/adb"),
            Path::new("/proc/modules"),
            Path::new("/sys/module"),
            crate::utils::ksu_next::is_runtime(),
        )
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    Ok(())
}

fn blocked(evidence: impl std::fmt::Display) -> Error {
    Error::msg(format!(
        "susfs_guard_lkm.ko (inforcqb/susfs4ksu-lkm) detected at {evidence}; \
         refusing Hybrid Mount installation/loading. Remove this LKM and reboot; \
         built-in SUSFS is supported"
    ))
}

fn probe_error(path: &Path, err: io::Error) -> Error {
    Error::msg(format!(
        "cannot check susfs4ksu-lkm conflict at {}: {err}",
        path.display()
    ))
}

fn check_at(adb: &Path, proc_modules: &Path, sys_modules: &Path) -> Result<()> {
    match fs::read_to_string(proc_modules) {
        Ok(text) => {
            if text
                .lines()
                .any(|line| line.split_whitespace().next() == Some(MODULE_NAME))
            {
                return Err(blocked(proc_modules.display()));
            }
        }
        // CONFIG_MODULES=n kernels have no module table; this is not a SUSFS ban.
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => return Err(probe_error(proc_modules, err)),
    }
    // initstate belongs to a dynamically loaded module. A sysfs directory alone
    // can also represent built-in code and must not trigger the ban.
    let initstate = sys_modules.join(MODULE_NAME).join("initstate");
    match fs::metadata(&initstate) {
        Ok(_) => return Err(blocked(initstate.display())),
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => return Err(probe_error(&initstate, err)),
    }
    // Include the project's documented /data/adb/loader location as well as
    // installed and staged root modules. Do not scan unrelated storage/backups.
    for directory in ["modules", "modules_update", "loader"] {
        check_objects(&adb.join(directory))?;
    }
    Ok(())
}

fn check_for_runtime(
    adb: &Path,
    proc_modules: &Path,
    sys_modules: &Path,
    next: bool,
) -> Result<()> {
    if next {
        log::info!("KernelSU Next runtime confirmed; skipping susfs4ksu-lkm conflict check");
        return Ok(());
    }
    check_at(adb, proc_modules, sys_modules)
}

fn check_objects(root: &Path) -> Result<()> {
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(probe_error(&directory, err)),
        };
        for entry in entries {
            let entry = entry.map_err(|err| probe_error(&directory, err))?;
            let path = entry.path();
            let kind = entry.file_type().map_err(|err| probe_error(&path, err))?;
            if kind.is_dir() {
                pending.push(path);
            } else if let Some(name) = entry.file_name().to_str()
                && (name == "susfs_guard_lkm.ko"
                    || (name.starts_with("susfs_guard_lkm-") && name.ends_with(".ko")))
            {
                return Err(blocked(path.display()));
            }
            // Do not recurse through symlink directories: module trees may
            // contain links outside the scan roots or cycles.
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Fixture;
    use std::fs;

    #[test]
    fn confirmed_next_skips_the_lkm_ban_even_if_probe_paths_are_unreadable() {
        let root = device();
        // A directory cannot be read as the /proc/modules text table.
        check_for_runtime(
            &root.join("adb"),
            &root.join("sys"),
            &root.join("sys"),
            true,
        )
        .unwrap();
    }

    #[test]
    fn other_or_unknown_runtimes_keep_the_lkm_ban() {
        let root = device();
        fs::write(root.join("modules"), "susfs_guard_lkm 12345 0 - Live 0x0\n").unwrap();
        assert!(
            check_for_runtime(
                &root.join("adb"),
                &root.join("modules"),
                &root.join("sys"),
                false
            )
            .is_err()
        );
    }

    fn device() -> Fixture {
        let root = Fixture::new("conflicts");
        fs::create_dir(root.join("adb")).unwrap();
        fs::create_dir(root.join("sys")).unwrap();
        fs::write(root.join("modules"), "kernelsu 16384 0 - Live 0x0\n").unwrap();
        root
    }

    fn check(root: &Path) -> Result<()> {
        check_at(&root.join("adb"), &root.join("modules"), &root.join("sys"))
    }

    #[test]
    fn rejects_installed_staged_and_loader_objects_including_release_names() {
        for relative in [
            "modules/susfs/kernel/susfs_guard_lkm.ko",
            "modules_update/susfs/susfs_guard_lkm.ko",
            "loader/susfs_guard_lkm.ko",
            "loader/susfs_guard_lkm-android13-5.15.ko",
        ] {
            let root = device();
            let object = root.join("adb").join(relative);
            fs::create_dir_all(object.parent().unwrap()).unwrap();
            fs::write(&object, "").unwrap();
            let err = check(&root).unwrap_err();
            assert!(err.to_string().contains("susfs_guard_lkm"), "{err}");
            assert!(
                err.to_string()
                    .contains(relative.split('/').next().unwrap()),
                "{err}"
            );
        }
    }

    #[test]
    fn rejects_loaded_module_even_when_object_was_deleted() {
        let root = device();
        fs::write(
            root.join("modules"),
            "susfs_guard_lkm 123456 0 - Live 0x0\n",
        )
        .unwrap();
        assert!(check(&root).is_err());
    }

    #[test]
    fn rejects_sysfs_loadable_module_when_proc_table_omits_it() {
        let root = device();
        fs::create_dir(root.join("sys/susfs_guard_lkm")).unwrap();
        fs::write(root.join("sys/susfs_guard_lkm/initstate"), "live\n").unwrap();
        assert!(check(&root).is_err());
    }

    #[test]
    fn allows_builtin_susfs_and_unrelated_names() {
        let root = device();
        fs::create_dir_all(root.join("sys/susfs/parameters")).unwrap();
        fs::create_dir(root.join("sys/susfs_guard_lkm")).unwrap();
        fs::create_dir_all(root.join("adb/modules/susfs")).unwrap();
        for name in ["susfs.ko", "susfs_guard__lkm.ko", "susfs_guard_lkm.ko.bak"] {
            fs::write(root.join("adb/modules/susfs").join(name), "").unwrap();
        }
        fs::write(
            root.join("modules"),
            "other_susfs_guard_lkm 10 0 - Live 0x0\n",
        )
        .unwrap();
        check(&root).unwrap();
        // A kernel built without module support has no /proc/modules.
        fs::remove_file(root.join("modules")).unwrap();
        check(&root).unwrap();
    }

    #[test]
    fn unreadable_module_table_is_not_treated_as_absent() {
        let root = device();
        fs::remove_file(root.join("modules")).unwrap();
        fs::create_dir(root.join("modules")).unwrap();
        assert!(check(&root).is_err());
    }
}
