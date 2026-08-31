// SPDX-License-Identifier: GPL-3.0-only

//! 文件系统辅助:路径清理、内核配置读取、tmpfs xattr 能力探测。

use std::path::Path;

#[cfg(unix)]
use std::fs::{self, OpenOptions};
#[cfg(unix)]
use std::io::Write;
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt, symlink};
#[cfg(unix)]
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

#[cfg(any(target_os = "linux", target_os = "android"))]
use rustix::fs::{AtFlags, CWD, FileType, Gid, Mode, Uid, chown, chownat, mknodat};
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::sync::atomic::{AtomicU8, Ordering};

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::config::Mode as MountMode;
#[cfg(unix)]
use crate::errors::Error;
use crate::errors::Result;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::mount_tree::{MountNode, MountTree, NodeFileType};

#[cfg(unix)]
static ATOMIC_WRITE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Replace a file without exposing a truncated intermediate state.
#[cfg(unix)]
pub fn atomic_write(path: &Path, content: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let sequence = ATOMIC_WRITE_SEQUENCE.fetch_add(1, AtomicOrdering::Relaxed);
    let temporary = parent.join(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        sequence
    ));

    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(content)?;
        file.sync_all()?;

        match fs::metadata(path) {
            Ok(metadata) => fs::set_permissions(&temporary, metadata.permissions())?,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }

        fs::rename(&temporary, path)?;
        // 父目录 fsync 失败按保存失败处理。rename 已经可见，但调用方
        // 必须知道目录项未持久化，不能把它当成一次成功的原子保存。
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(not(unix))]
pub fn atomic_write(path: &Path, content: &[u8]) -> Result<()> {
    // Host 测试/开发用途的非原子回退：Windows 上不提供崩溃安全的
    // 临时文件 + rename 语义，发布目标(Android/Linux)始终走上面的实现。
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(())
}

/// 删除路径:目录递归删除,非目录直接删除,不存在视为成功。
pub fn remove_path(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => Ok(std::fs::remove_dir_all(path)?),
        Ok(_) => Ok(std::fs::remove_file(path)?),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CopyTreeStats {
    pub directories: usize,
    pub files: usize,
    pub symlinks: usize,
    pub special_entries: usize,
    pub opaque_directories: usize,
    pub bytes: u64,
}

/// 从 planner 的共享节点树物化 OverlayFS 层。只复制标注为 overlay 的贡献，
/// magic / ignore 子树不会泄漏进 lowerdir；模块源目录始终只读。
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn stage_overlay_tree(tree: &MountTree, destination: &Path) -> Result<CopyTreeStats> {
    for module_id in tree.module_ids_for(MountMode::Overlay) {
        remove_path(&destination.join(module_id.as_str()))?;
    }

    let mut stats = CopyTreeStats::default();
    let mut directory_metadata = Vec::new();
    stage_overlay_node(&tree.root, destination, &mut stats, &mut directory_metadata)?;

    // 子节点创建完成后再恢复目录元数据，避免 staging 写入改变最终属性。
    for (source, staged, metadata) in directory_metadata.into_iter().rev() {
        clone_entry_metadata(&source, &staged, &metadata, false);
    }

    Ok(stats)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn stage_overlay_node(
    node: &MountNode,
    destination: &Path,
    stats: &mut CopyTreeStats,
    directory_metadata: &mut Vec<(std::path::PathBuf, std::path::PathBuf, fs::Metadata)>,
) -> Result<()> {
    for source in node
        .sources
        .iter()
        .filter(|source| source.backend == MountMode::Overlay)
    {
        let staged = source.relative.split('/').fold(
            destination.join(source.module_id.as_str()),
            |path, component| path.join(component),
        );
        let metadata = fs::symlink_metadata(&source.source_path)?;

        if source.file_type != NodeFileType::Directory
            && let Some(parent) = staged.parent()
        {
            fs::create_dir_all(parent)?;
        }

        match source.file_type {
            NodeFileType::Directory => {
                fs::create_dir_all(&staged)?;
                stats.directories += 1;
                if source.replace {
                    set_overlay_opaque(&staged)?;
                    stats.opaque_directories += 1;
                }
                directory_metadata.push((source.source_path.clone(), staged, metadata));
                continue;
            }
            NodeFileType::RegularFile => {
                fs::copy(&source.source_path, &staged)?;
                stats.files += 1;
                stats.bytes = stats.bytes.saturating_add(metadata.len());
            }
            NodeFileType::Symlink => {
                symlink(fs::read_link(&source.source_path)?, &staged)?;
                stats.symlinks += 1;
            }
            NodeFileType::Whiteout => {
                make_device_node(&staged, &metadata)?;
                stats.special_entries += 1;
            }
        }

        clone_entry_metadata(
            &source.source_path,
            &staged,
            &metadata,
            source.file_type == NodeFileType::Symlink,
        );
    }

    for child in node.children.values() {
        stage_overlay_node(child, destination, stats, directory_metadata)?;
    }
    Ok(())
}

/// 复制已经物化的单个 Overlay 节点到 shallow layer，保留符号链接、
/// whiteout 设备节点、权限、所有权和 SELinux 上下文。
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn copy_prepared_entry(source: &Path, destination: &Path) -> Result<()> {
    let mut stats = CopyTreeStats::default();
    copy_tree_entry(source, destination, &mut stats)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn copy_tree_entry(source: &Path, destination: &Path, stats: &mut CopyTreeStats) -> Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    let file_type = metadata.file_type();

    if file_type.is_dir() {
        fs::create_dir_all(destination)?;
        stats.directories += 1;

        for entry in fs::read_dir(source)? {
            let entry = entry?;
            if entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(crate::defs::REPLACE_DIR_FILE_NAME)
            {
                set_overlay_opaque(destination)?;
                stats.opaque_directories += 1;
                continue;
            }
            copy_tree_entry(&entry.path(), &destination.join(entry.file_name()), stats)?;
        }

        clone_entry_metadata(source, destination, &metadata, false);
        return Ok(());
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }

    if file_type.is_symlink() {
        symlink(fs::read_link(source)?, destination)?;
        stats.symlinks += 1;
    } else if file_type.is_file() {
        fs::copy(source, destination)?;
        stats.files += 1;
        stats.bytes = stats.bytes.saturating_add(metadata.len());
    } else if file_type.is_char_device() || file_type.is_block_device() || file_type.is_fifo() {
        make_device_node(destination, &metadata)?;
        stats.special_entries += 1;
    } else {
        return Err(Error::msg(format!(
            "unsupported module entry type: {}",
            source.display()
        )));
    }

    clone_entry_metadata(source, destination, &metadata, file_type.is_symlink());
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn set_overlay_opaque(path: &Path) -> Result<()> {
    crate::utils::write_xattr(path, crate::defs::REPLACE_DIR_XATTR, b"y").map_err(|err| {
        Error::msg(format!(
            "set overlay opaque xattr on {}: {err}",
            path.display()
        ))
    })?;
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn make_device_node(path: &Path, metadata: &fs::Metadata) -> Result<()> {
    let raw_mode = metadata.permissions().mode();
    let file_type = FileType::from_raw_mode(raw_mode);
    if matches!(file_type, FileType::Unknown) {
        return Err(Error::msg(format!(
            "cannot recreate special module entry {}: unknown type",
            path.display()
        )));
    }

    mknodat(
        CWD,
        path,
        file_type,
        Mode::from_raw_mode(raw_mode & 0o7777),
        metadata.rdev() as _,
    )?;
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn clone_entry_metadata(
    source: &Path,
    destination: &Path,
    metadata: &fs::Metadata,
    is_symlink: bool,
) {
    if !is_symlink && let Err(err) = fs::set_permissions(destination, metadata.permissions()) {
        log::warn!(
            "copy metadata permissions skipped: src={}, dst={}, error={err}",
            source.display(),
            destination.display()
        );
    }

    let ownership_result = if is_symlink {
        chownat(
            CWD,
            destination,
            Some(Uid::from_raw(metadata.uid())),
            Some(Gid::from_raw(metadata.gid())),
            AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(std::io::Error::from)
    } else {
        chown(
            destination,
            Some(Uid::from_raw(metadata.uid())),
            Some(Gid::from_raw(metadata.gid())),
        )
        .map_err(std::io::Error::from)
    };

    if let Err(err) = ownership_result {
        log::warn!(
            "copy metadata ownership skipped: src={}, dst={}, uid={}, gid={}, error={err}",
            source.display(),
            destination.display(),
            metadata.uid(),
            metadata.gid()
        );
    }

    if let Ok(context) = crate::utils::lgetfilecon(source)
        && let Err(err) = crate::utils::lsetfilecon(destination, &context)
    {
        log::warn!(
            "copy metadata SELinux context skipped: src={}, dst={}, error={err}",
            source.display(),
            destination.display()
        );
    }
}

/// Make an OverlayFS layer root behave like the stock directory it will cover.
/// Unlike ordinary module-entry staging, every part of this metadata is
/// required: a private 0700 directory or a wrongly labeled OverlayFS root can
/// make an entire Android partition inaccessible after the mount.
#[cfg(unix)]
fn directory_metadata(source: &Path) -> Result<fs::Metadata> {
    // Follow the final symlink: Android partition layouts may expose a stock
    // overlay target such as /system/media as a symlink to the real directory.
    let metadata = fs::metadata(source)?;
    if !metadata.file_type().is_dir() {
        return Err(Error::msg(format!(
            "overlay layer metadata source is not a directory: {}",
            source.display()
        )));
    }
    Ok(metadata)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn clone_directory_metadata(source: &Path, destination: &Path) -> Result<()> {
    let metadata = directory_metadata(source)?;

    chown(
        destination,
        Some(Uid::from_raw(metadata.uid())),
        Some(Gid::from_raw(metadata.gid())),
    )
    .map_err(|err| {
        Error::msg(format!(
            "copy overlay layer ownership {} -> {}: {err}",
            source.display(),
            destination.display()
        ))
    })?;
    fs::set_permissions(destination, metadata.permissions()).map_err(|err| {
        Error::msg(format!(
            "copy overlay layer permissions {} -> {}: {err}",
            source.display(),
            destination.display()
        ))
    })?;

    let context = crate::utils::lgetfilecon(source)?;
    crate::utils::lsetfilecon(destination, &context).map_err(|err| {
        Error::msg(format!(
            "copy overlay layer SELinux context {} -> {}: {err}",
            source.display(),
            destination.display()
        ))
    })?;

    log::debug!(
        "overlay layer directory metadata cloned: src={}, dst={}, mode={:o}, uid={}, gid={}, context={}",
        source.display(),
        destination.display(),
        metadata.permissions().mode() & 0o7777,
        metadata.uid(),
        metadata.gid(),
        context
    );
    Ok(())
}

/// 读取 `/proc/config.gz`,检查 `CONFIG_*` 是否编译为 `y`(v4.2.0 行为)。
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn check_kernel_config(key: &str) -> Result<bool> {
    use std::io::Read;

    use flate2::read::GzDecoder;

    let file = std::fs::File::open("/proc/config.gz")?;
    let mut config = String::new();
    GzDecoder::new(file).read_to_string(&mut config)?;

    let found = config.lines().any(|line| {
        if line.starts_with('#') {
            return false;
        }
        let Some((name, value)) = line.split_once('=') else {
            return false;
        };
        name.trim() == key && value.trim() == "y"
    });

    Ok(found)
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn check_kernel_config(_key: &str) -> Result<bool> {
    Ok(false)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
static TMPFS_XATTR_SUPPORT: AtomicU8 = AtomicU8::new(0);

/// overlay 层落到 tmpfs 时要求 tmpfs 支持 xattr;结果缓存一次(v4.2.0 行为)。
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn is_overlay_xattr_supported() -> Result<bool> {
    match TMPFS_XATTR_SUPPORT.load(Ordering::Relaxed) {
        1 => return Ok(false),
        2 => return Ok(true),
        _ => {}
    }

    let supported = check_kernel_config("CONFIG_TMPFS_XATTR")?;
    TMPFS_XATTR_SUPPORT.store(if supported { 2 } else { 1 }, Ordering::Relaxed);
    Ok(supported)
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn is_overlay_xattr_supported() -> Result<bool> {
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_path_handles_missing_and_files() {
        let dir = std::env::temp_dir().join(format!("hybrid-mount-remove-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.txt");
        std::fs::write(&file, "x").unwrap();

        remove_path(&dir.join("missing")).unwrap();
        remove_path(&file).unwrap();
        assert!(!file.exists());
        remove_path(&dir).unwrap();
        assert!(!dir.exists());
    }

    #[test]
    fn atomic_write_creates_and_replaces_content() {
        let dir = std::env::temp_dir().join(format!("hybrid-mount-atomic-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("state.txt");

        atomic_write(&path, b"first").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"first");
        atomic_write(&path, b"second").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"second");

        let leftovers = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        assert_eq!(leftovers, 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_removes_temp_file_when_rename_fails() {
        let dir =
            std::env::temp_dir().join(format!("hybrid-mount-atomic-fail-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("target");
        std::fs::create_dir_all(&target).unwrap();

        let err = atomic_write(&target, b"payload").unwrap_err();
        let crate::errors::Error::Io(source) = err else {
            panic!("atomic_write failure must wrap an I/O error");
        };
        assert_ne!(source.kind(), std::io::ErrorKind::NotFound);

        assert!(target.is_dir());
        let leftovers = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        assert_eq!(leftovers, 0, "rename 失败后不能遗留临时文件");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn atomic_write_fails_when_parent_is_not_a_directory() {
        let dir =
            std::env::temp_dir().join(format!("hybrid-mount-parent-file-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let parent_file = dir.join("parent");
        std::fs::write(&parent_file, b"occupied").unwrap();
        let target = parent_file.join("config.toml");

        let err = atomic_write(&target, b"payload").unwrap_err();
        let crate::errors::Error::Io(source) = err else {
            panic!("atomic_write failure must wrap an I/O error");
        };
        assert_ne!(source.kind(), std::io::ErrorKind::NotFound);

        assert_eq!(std::fs::read(&parent_file).unwrap(), b"occupied");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn shared_tree_staging_keeps_overlay_nodes_and_excludes_magic_nodes() {
        use crate::mount_tree::MountSource;

        let fixture = std::env::temp_dir().join(format!(
            "hybrid-mount-shared-tree-stage-{}",
            std::process::id()
        ));
        remove_path(&fixture).unwrap();
        let module = fixture.join("source/m");
        let etc = module.join("system/etc");
        fs::create_dir_all(&etc).unwrap();
        fs::write(etc.join("overlay.conf"), "overlay").unwrap();
        fs::write(etc.join("magic.conf"), "magic").unwrap();
        symlink("overlay.conf", etc.join("overlay.link")).unwrap();

        let source = |relative: &str, file_type: NodeFileType, backend: MountMode| MountSource {
            module_id: crate::module_id::ModuleId::try_from("m").unwrap(),
            relative: relative.to_owned(),
            source_path: relative
                .split('/')
                .fold(module.clone(), |path, component| path.join(component)),
            file_type,
            replace: false,
            backend,
        };
        let mut tree = MountTree::default();
        tree.insert(
            "/system/etc",
            source("system/etc", NodeFileType::Directory, MountMode::Overlay),
        );
        tree.insert(
            "/system/etc/overlay.conf",
            source(
                "system/etc/overlay.conf",
                NodeFileType::RegularFile,
                MountMode::Overlay,
            ),
        );
        tree.insert(
            "/system/etc/overlay.link",
            source(
                "system/etc/overlay.link",
                NodeFileType::Symlink,
                MountMode::Overlay,
            ),
        );
        tree.insert(
            "/system/etc/magic.conf",
            source(
                "system/etc/magic.conf",
                NodeFileType::RegularFile,
                MountMode::Magic,
            ),
        );

        let staging = fixture.join("staging");
        let stats = stage_overlay_tree(&tree, &staging).unwrap();
        let staged_etc = staging.join("m/system/etc");
        assert_eq!(
            fs::read_to_string(staged_etc.join("overlay.conf")).unwrap(),
            "overlay"
        );
        assert_eq!(
            fs::read_link(staged_etc.join("overlay.link")).unwrap(),
            Path::new("overlay.conf")
        );
        assert!(!staged_etc.join("magic.conf").exists());
        assert_eq!(stats.files, 1);
        assert_eq!(stats.symlinks, 1);

        remove_path(&fixture).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn directory_metadata_follows_symlink_to_stock_directory() {
        use std::os::unix::fs::symlink;

        let fixture = std::env::temp_dir().join(format!(
            "hybrid-mount-directory-metadata-symlink-{}",
            std::process::id()
        ));
        let stock = fixture.join("stock/media");
        let target = fixture.join("target/media");
        remove_path(&fixture).unwrap();
        std::fs::create_dir_all(&stock).unwrap();
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        symlink("../stock/media", &target).unwrap();

        assert!(directory_metadata(&target).unwrap().is_dir());

        remove_path(&fixture).unwrap();
    }
}
