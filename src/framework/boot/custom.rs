use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use rustix::mount::{MountFlags, UnmountFlags, mount_bind, mount_move, mount_remount, unmount};

use super::{MagicStagingGuard, MountedTargets};
use crate::errors::{Error, Result};
use crate::framework::config::Config;
use crate::framework::extension::{HookMount, MountKind};
use crate::mount::overlayfs::exec::{
    MountEffect, bind_mount, cleanup_staging_mount, mount_overlay,
};
use crate::mount::overlayfs::layers::{
    OverlayDirectoryTarget, overlay_mount_source, prepare_shallow_destination,
    resolve_overlay_directory_target,
};
use crate::mount::overlayfs::staging::copy_prepared_entry;
use crate::platform::sys::fs::clone_directory_metadata;
use crate::platform::sys::temp::{RuntimeTempDir, create_random_dir};
use crate::platform::sys::transaction::MountTransaction;

pub(super) struct HookEffects {
    pub binds: Vec<String>,
    pub overlays: Vec<String>,
    pub vfs: Vec<String>,
    pub provider: Option<&'static str>,
    pub mount_source: &'static str,
}

impl HookEffects {
    pub fn apply(
        &mut self,
        commands: Vec<HookMount>,
        config: &Config,
        transaction: &mut MountTransaction<'_>,
        mounted: &mut MountedTargets,
    ) -> Result<()> {
        for command in commands {
            self.mount(&command, config, transaction, mounted)
                .map_err(|err| {
                    Error::msg(format!(
                        "{} ({:?} {:?}) {} -> {}: {err}",
                        command.script.display(),
                        command.phase,
                        command.kind,
                        command.source.display(),
                        command.target.display()
                    ))
                })?;
        }
        Ok(())
    }

    fn mount(
        &mut self,
        command: &HookMount,
        config: &Config,
        transaction: &mut MountTransaction<'_>,
        mounted: &mut MountedTargets,
    ) -> Result<()> {
        let target = &command.target;
        if command.kind == MountKind::Vfs && self.provider.is_none() {
            if config.vfs_strict {
                return Err(Error::msg(
                    "Lua vfs_mount requires an available VFS provider",
                ));
            }
            log::warn!(
                "skipping Lua vfs_mount for {}: VFS provider unavailable",
                target.display()
            );
            return Ok(());
        }
        let source = match command.source.canonicalize() {
            Ok(source) => source,
            Err(err) if err.kind() == ErrorKind::NotFound && command.kind == MountKind::Bind => {
                log::warn!(
                    "skipping Lua bind_mount with missing source {}",
                    command.source.display()
                );
                return Ok(());
            }
            Err(err) => return Err(err.into()),
        };
        let metadata = fs::metadata(&source)?;
        if !metadata.is_file() && !metadata.is_dir() {
            return Err(Error::msg("expected a regular file or directory source"));
        }
        let actual = match command.kind {
            MountKind::Vfs => {
                let target = target
                    .to_str()
                    .ok_or_else(|| Error::msg("VFS target requires UTF-8"))?;
                let provider = self
                    .provider
                    .ok_or_else(|| Error::msg("VFS provider unavailable"))?;
                crate::framework::runtime::hook_vfs::apply(
                    &source,
                    target,
                    provider,
                    &config.vfs_isolate_uids,
                    transaction,
                )?;
                self.vfs.push(target.to_owned());
                return Ok(());
            }
            MountKind::Bind => {
                let actual = bind(&source, target, self.mount_source, transaction, mounted)?;
                self.binds.push(actual.display().to_string());
                actual
            }
            MountKind::Overlay => {
                let actual = overlay(
                    &source,
                    target,
                    self.mount_source,
                    !config.disable_umount,
                    transaction,
                    mounted,
                )?;
                self.overlays.push(actual.display().to_string());
                actual
            }
        };
        if !config.disable_umount {
            crate::platform::ksu::send_unmountable(&actual);
        }
        Ok(())
    }
}

fn record(target: &Path, transaction: &mut MountTransaction<'_>, mounted: &mut MountedTargets) {
    mounted.paths.push(target.display().to_string());
    let path = target.to_path_buf();
    transaction.register_rollback_only(format!("Lua mount {}", target.display()), move || {
        unmount(&path, UnmountFlags::DETACH).map_err(Error::from)
    });
}

fn staging(mount_source: &str, transaction: &mut MountTransaction<'_>) -> Result<PathBuf> {
    let mut session = RuntimeTempDir::create()?;
    let path = create_random_dir(session.path())?;
    let guard = MagicStagingGuard::mount(path.clone(), mount_source)?;
    session.keep();
    transaction.register("Lua staging", move || {
        guard.cleanup()?;
        session.cleanup_unconditional()
    });
    Ok(path)
}

fn mount_target(path: &Path) -> Result<PathBuf> {
    let canonical = path.canonicalize()?;
    if canonical.parent().is_none() {
        return Err(Error::msg("refusing to mount over /"));
    }
    Ok(canonical)
}

fn bind(
    source: &Path,
    target: &Path,
    mount_source: &str,
    transaction: &mut MountTransaction<'_>,
    mounted: &mut MountedTargets,
) -> Result<PathBuf> {
    match fs::symlink_metadata(target) {
        Ok(_) => {
            let actual = mount_target(target)?;
            mount_bind(source, &actual)?;
            record(&actual, transaction, mounted);
            crate::platform::sys::mount::normalize_propagation(&actual, false)?;
            mount_remount(&actual, MountFlags::BIND | MountFlags::RDONLY, "")?;
            return Ok(actual);
        }
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => return Err(err.into()),
    }
    let OverlayDirectoryTarget::Missing {
        mount_target: ancestor,
        destination_relative,
    } = resolve_overlay_directory_target(target)?
    else {
        return Err(Error::msg("bind target appeared during preparation"));
    };
    let ancestor = mount_target(&ancestor)?;
    let staging_root = staging(mount_source, transaction)?;
    let stage = create_random_dir(&staging_root)?;
    if stage.starts_with(&ancestor) || stage.starts_with(source) {
        return Err(Error::msg(
            "bind source or target contains the staging directory",
        ));
    }
    mount_bind(&stage, &stage)?;
    let destination = prepare_shallow_destination(&ancestor, &stage, &destination_relative)?;
    for entry in ancestor.read_dir()? {
        let entry = entry?;
        let original = entry.path();
        let mirror = stage.join(entry.file_name());
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            copy_prepared_entry(&original, &mirror)?;
            continue;
        }
        if kind.is_dir() {
            fs::create_dir(&mirror)?;
        } else if kind.is_file() {
            fs::File::create_new(&mirror)?;
        } else {
            return Err(Error::msg(format!("cannot mirror {}", original.display())));
        }
        bind_mount(&original, &mirror)?;
    }
    if source.is_dir() {
        fs::create_dir(&destination)?;
    } else {
        fs::File::create_new(&destination)?;
    }
    mount_bind(source, &destination)?;
    mount_remount(&destination, MountFlags::BIND | MountFlags::RDONLY, "")?;
    mount_remount(&stage, MountFlags::BIND | MountFlags::RDONLY, "")?;
    let snapshot = crate::platform::sys::mountinfo::MountSnapshot::read()?;
    let children = snapshot
        .descendants(&stage)
        .into_iter()
        .filter_map(|child| {
            child
                .strip_prefix(&stage)
                .ok()
                .map(|suffix| ancestor.join(suffix))
        });
    mount_move(&stage, &ancestor)?;
    record(&ancestor, transaction, mounted);
    mounted
        .paths
        .extend(children.map(|path| path.display().to_string()));
    Ok(ancestor)
}

fn overlay(
    source: &Path,
    target: &Path,
    mount_source: &str,
    register_unmountable: bool,
    transaction: &mut MountTransaction<'_>,
    mounted: &mut MountedTargets,
) -> Result<PathBuf> {
    let directory = if source.is_dir() {
        target
    } else {
        target
            .parent()
            .ok_or_else(|| Error::msg("missing overlay target parent"))?
    };
    let ancestor = match resolve_overlay_directory_target(directory)? {
        OverlayDirectoryTarget::Existing => directory.to_path_buf(),
        OverlayDirectoryTarget::Missing { mount_target, .. } => mount_target,
    };
    let suffix = target
        .strip_prefix(&ancestor)
        .map_err(|err| Error::msg(err.to_string()))?;
    let actual = mount_target(&ancestor)?;
    let stage = staging(mount_source, transaction)?;
    if stage.starts_with(source) || stage.starts_with(&actual) {
        return Err(Error::msg(
            "overlay source or target contains the staging directory",
        ));
    }
    let layer = stage.join("layer");
    if suffix.as_os_str().is_empty() {
        copy_prepared_entry(source, &layer)?;
        clone_directory_metadata(&actual, &layer)?;
    } else {
        fs::create_dir(&layer)?;
        let destination = prepare_shallow_destination(&actual, &layer, suffix)?;
        copy_prepared_entry(source, &destination)?;
    }
    let root = actual
        .to_str()
        .ok_or_else(|| Error::msg("overlay target requires UTF-8"))?;
    mount_overlay(
        root,
        &[layer.display().to_string()],
        None,
        None,
        &stage,
        overlay_mount_source(root, mount_source),
        register_unmountable,
        &mut |effect| match effect {
            MountEffect::Target(path) => record(Path::new(&path), transaction, mounted),
            MountEffect::Staging(path) => transaction
                .register("Lua overlay intermediate", move || {
                    cleanup_staging_mount(path)
                }),
        },
    )?;
    Ok(actual)
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use crate::framework::extension::{Extensions, HookPhase};
    use crate::test_support::{Fixture, require_mount_namespace};

    #[test]
    fn lua_bind_is_read_only_and_rollback_restores_the_stock_file() {
        if !require_mount_namespace() {
            return;
        }
        let root = Fixture::new("lua-bind");
        let target = root.join("target");
        fs::write(root.join("source"), "custom").unwrap();
        fs::write(&target, "stock").unwrap();
        fs::write(root.join("script.lua"), "API = 1\nfunction before() bind_mount(moduledir .. '/source', moduledir .. '/target') end").unwrap();
        let extensions = Extensions::load(&root, &root).unwrap();
        let mut effects = HookEffects {
            binds: Vec::new(),
            overlays: Vec::new(),
            vfs: Vec::new(),
            provider: None,
            mount_source: "KSU",
        };
        let mut transaction = MountTransaction::new();
        let mut mounted = MountedTargets::default();
        let config = Config {
            disable_umount: true,
            ..Config::default()
        };
        effects
            .apply(
                extensions.commands(HookPhase::Before).unwrap(),
                &config,
                &mut transaction,
                &mut mounted,
            )
            .unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "custom");
        assert!(fs::write(&target, "changed").is_err());
        assert_eq!(effects.binds, mounted.paths);
        assert!(transaction.rollback().failures.is_empty());
        assert_eq!(fs::read_to_string(target).unwrap(), "stock");
    }
}
