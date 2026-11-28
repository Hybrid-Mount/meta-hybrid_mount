use std::fs;
use std::os::unix::fs::{FileTypeExt, symlink};
use std::path::Path;

use rustix::fs::{CWD, FileType, Mode, mknodat};

use crate::errors::{Error, Result};
use crate::mount::model::{Mode as MountMode, MountNode, MountTree, NodeFileType};
use crate::platform::sys::fs::{clone_entry_metadata, make_device_node, remove_path};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CopyTreeStats {
    pub directories: usize,
    pub files: usize,
    pub symlinks: usize,
    pub special_entries: usize,
    pub opaque_directories: usize,
    pub bytes: u64,
}

/// Materialises OverlayFS layers from the planner's shared node tree. Only overlay-labelled
/// contributions are copied, so magic and ignore subtrees cannot leak into a lowerdir, and module sources stay read-only.
pub fn stage_overlay_tree(tree: &MountTree, destination: &Path) -> Result<CopyTreeStats> {
    for module_id in tree.module_ids_for(MountMode::Overlay) {
        remove_path(&destination.join(module_id.as_str()))?;
    }

    let mut stats = CopyTreeStats::default();
    let mut directory_metadata = Vec::new();
    stage_overlay_node(&tree.root, destination, &mut stats, &mut directory_metadata)?;

    // Restore directory metadata only after the children exist, so staging writes cannot change the final attributes.
    for (source, staged, metadata) in directory_metadata.into_iter().rev() {
        clone_entry_metadata(&source, &staged, &metadata, false);
    }

    Ok(stats)
}

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
                // A directory .remove is a logical whiteout whose read-only
                // source remains a directory. Materialize an actual 0:0 node.
                if metadata.is_dir() {
                    mknodat(
                        CWD,
                        &staged,
                        FileType::CharacterDevice,
                        Mode::from_raw_mode(0o600),
                        0,
                    )?;
                } else {
                    make_device_node(&staged, &metadata)?;
                }
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

/// Copies one materialised Overlay node into a shallow layer, preserving symlinks,
/// whiteout device nodes, permissions, ownership and the SELinux context.
pub fn copy_prepared_entry(source: &Path, destination: &Path) -> Result<()> {
    let mut stats = CopyTreeStats::default();
    copy_tree_entry(source, destination, &mut stats)
}

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

fn set_overlay_opaque(path: &Path) -> Result<()> {
    crate::platform::write_xattr(path, crate::defs::REPLACE_DIR_XATTR, b"y").map_err(|err| {
        Error::msg(format!(
            "set overlay opaque xattr on {}: {err}",
            path.display()
        ))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_remove_stages_a_whiteout_without_copying_its_contents() {
        use std::os::unix::fs::{FileTypeExt, MetadataExt};
        let fixture = crate::test_support::Fixture::new("directory-remove-stage");
        let source = fixture.join("module/system/app/Foo");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join(".remove"), "").unwrap();
        fs::write(source.join("keep-on-disk"), "original").unwrap();
        let mut tree = MountTree::default();
        tree.insert(
            "/system/app/Foo",
            crate::test_support::mount_source_at(
                "m",
                "system/app/Foo",
                NodeFileType::Whiteout,
                MountMode::Overlay,
                source.clone(),
            ),
        );
        let staging = fixture.join("staging");
        stage_overlay_tree(&tree, &staging).unwrap();
        let metadata = fs::symlink_metadata(staging.join("m/system/app/Foo")).unwrap();
        assert!(metadata.file_type().is_char_device());
        assert_eq!(metadata.rdev(), 0);
        assert_eq!(
            fs::read_to_string(source.join("keep-on-disk")).unwrap(),
            "original"
        );
    }

    #[test]
    fn shared_tree_staging_keeps_overlay_nodes_and_excludes_magic_nodes() {
        let fixture = crate::test_support::Fixture::new("shared-tree-stage");
        let module = fixture.join("source/m");
        let etc = module.join("system/etc");
        fs::create_dir_all(&etc).unwrap();
        fs::write(etc.join("overlay.conf"), "overlay").unwrap();
        fs::write(etc.join("magic.conf"), "magic").unwrap();
        symlink("overlay.conf", etc.join("overlay.link")).unwrap();

        let source = |relative: &str, file_type, backend| {
            crate::test_support::mount_source_at(
                "m",
                relative,
                file_type,
                backend,
                module.join(relative),
            )
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
    }
}
