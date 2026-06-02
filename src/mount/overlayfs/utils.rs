// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#[cfg(any(target_os = "linux", target_os = "android"))]
use std::fs;
use std::path::Path;
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::{os::fd::AsFd, os::unix::fs::PermissionsExt};

#[cfg(any(target_os = "linux", target_os = "android"))]
use anyhow::Context;
use anyhow::Result;
#[cfg(any(target_os = "linux", target_os = "android"))]
use loopdev::LoopControl;
#[cfg(any(target_os = "linux", target_os = "android"))]
use rustix::mount::{MountFlags, UnmountFlags, mount, unmount};
#[cfg(any(target_os = "linux", target_os = "android"))]
use rustix::{
    fs::CWD,
    mount::{
        FsMountFlags, FsOpenFlags, MountAttrFlags, MoveMountFlags, fsconfig_create,
        fsconfig_set_string, fsmount, fsopen, move_mount,
    },
};

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn mount_ext4<P>(source: P, target: P) -> Result<()>
where
    P: AsRef<Path>,
{
    let path = source.as_ref();
    if !path.exists() {
        crate::scoped_log!(warn, "overlayfs:utils", "source path does not exist");
    } else {
        let metadata = fs::metadata(path)?;
        let permissions = metadata.permissions();
        let mode = permissions.mode();

        if permissions.readonly() {
            crate::scoped_log!(
                debug,
                "overlayfs:utils",
                "file permissions(octal): {:o}",
                mode & 0o777
            );
        }
    }

    mount_ext4_loop(source.as_ref(), target.as_ref())?;
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn mount_ext4<P>(_source: P, _target: P) -> Result<()>
where
    P: AsRef<Path>,
{
    anyhow::bail!("ext4 mounting is only supported on linux/android")
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn mount_ext4_loop<P>(source: P, target: P) -> Result<()>
where
    P: AsRef<Path>,
{
    let lc = LoopControl::open().context("Failed to open loop control")?;
    let ld = lc.next_free().context("Failed to find free loop device")?;

    ld.with()
        .read_only(false)
        .autoclear(true)
        .attach(source.as_ref())
        .context("Failed to attach source to loop device")?;

    let device_path = ld.path().context("Could not get loop device path")?;
    crate::scoped_log!(
        debug,
        "overlayfs:utils",
        "loop device: path={}",
        device_path.display()
    );

    mount(
        &device_path,
        target.as_ref(),
        "ext4",
        MountFlags::NOATIME,
        Some(c""),
    )
    .context(format!(
        "Failed to mount {} to {}",
        device_path.display(),
        target.as_ref().display()
    ))?;

    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn is_overlay_supported() -> Result<bool> {
    crate::sys::fs::check_kernel_config("CONFIG_OVERLAY_FS")
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn is_overlay_supported() -> Result<bool> {
    Ok(false)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn umount_dir(src: impl AsRef<Path>) -> Result<()> {
    unmount(src.as_ref(), UnmountFlags::empty())
        .with_context(|| format!("Failed to umount {}", src.as_ref().display()))?;
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn umount_dir(_src: impl AsRef<Path>) -> Result<()> {
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn fs<S, P>(
    upperdir: Option<String>,
    workdir: Option<String>,
    lowerdir_config: String,
    source: S,
    dest: P,
) -> Result<()>
where
    S: ToString,
    P: AsRef<Path>,
{
    let fs = fsopen("overlay", FsOpenFlags::FSOPEN_CLOEXEC).context("Failed to fsopen overlay")?;
    let fs = fs.as_fd();
    fsconfig_set_string(fs, "lowerdir", &lowerdir_config).with_context(|| {
        format!("Failed to fsconfig set string lowerdir with {lowerdir_config}")
    })?;
    if let (Some(upperdir), Some(workdir)) = (&upperdir, &workdir) {
        fsconfig_set_string(fs, "upperdir", upperdir)
            .with_context(|| format!("Failed to fsconfig set string upperdir with {upperdir}"))?;
        fsconfig_set_string(fs, "workdir", workdir)
            .with_context(|| format!("Failed to fsconfig set string workdir with {workdir}"))?;
    }
    let source_s = source.to_string();
    fsconfig_set_string(fs, "source", &source_s)
        .with_context(|| format!("Failed to fsconfig set string source with {source_s}"))?;
    fsconfig_create(fs).context("Failed to fsconfig create new fs")?;
    let mount = fsmount(fs, FsMountFlags::FSMOUNT_CLOEXEC, MountAttrFlags::empty())
        .context("Failed to mount")?;
    move_mount(
        mount.as_fd(),
        "",
        CWD,
        dest.as_ref(),
        MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH,
    )?;

    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn fs<S, P>(
    _upperdir: Option<String>,
    _workdir: Option<String>,
    _lowerdir_config: String,
    _source: S,
    _dest: P,
) -> Result<()>
where
    S: ToString,
    P: AsRef<Path>,
{
    anyhow::bail!("overlay fsopen mount is only supported on linux/android")
}
