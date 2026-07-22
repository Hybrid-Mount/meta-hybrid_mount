// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{
    collections::{BTreeMap, btree_map::Entry},
    fmt,
    fs::{DirEntry, FileType},
    os::unix::fs::{FileTypeExt, MetadataExt},
    path::{Path, PathBuf},
};

use anyhow::Result;

use crate::defs::REPLACE_DIR_FILE_NAME;

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub enum NodeFileType {
    RegularFile,
    Directory,
    Symlink,
    Whiteout,
}

impl From<FileType> for NodeFileType {
    fn from(value: FileType) -> Self {
        if value.is_file() {
            Self::RegularFile
        } else if value.is_dir() {
            Self::Directory
        } else if value.is_symlink() {
            Self::Symlink
        } else {
            Self::Whiteout
        }
    }
}

#[derive(Clone)]
pub struct Node {
    pub name: String,
    pub file_type: NodeFileType,
    pub children: BTreeMap<String, Self>,
    pub module_path: Option<PathBuf>,
    pub replace: bool,
    pub skip: bool,
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.debug_tree(f, 0)
    }
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Magic mount tree dump. Share '/data/adb/magic_mount/tree' with the developer for diagnostics."
        )
    }
}

impl Node {
    fn debug_tree(&self, f: &mut fmt::Formatter<'_>, indent: usize) -> fmt::Result {
        let indent_str = "  ".repeat(indent);

        write!(f, "{}{} ({:?})", indent_str, self.name, self.file_type)?;
        if let Some(path) = &self.module_path {
            write!(f, " [{}]", path.display())?;
        }
        if self.replace {
            write!(f, " [R]")?;
        }
        if self.skip {
            write!(f, " [S]")?;
        }
        writeln!(f)?;

        for child in self.children.values() {
            child.debug_tree(f, indent + 1)?;
        }
        Ok(())
    }

    pub fn collect_module_files<P>(&mut self, module_dir: P) -> Result<bool>
    where
        P: AsRef<Path>,
    {
        let dir = module_dir.as_ref();
        let mut has_file = false;
        for entry_result in dir.read_dir()? {
            let entry = entry_result?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| anyhow::anyhow!("non-UTF-8 entry under {}", dir.display()))?;

            let node = match self.children.entry(name.clone()) {
                Entry::Occupied(o) => o.into_mut(),
                Entry::Vacant(v) => v.insert(Self::new_module(&name, &entry)?),
            };

            has_file |= if node.file_type == NodeFileType::Directory {
                node.collect_module_files(dir.join(&node.name))? || node.replace
            } else {
                true
            }
        }

        Ok(has_file)
    }

    fn dir_is_replace<P>(path: P) -> Result<bool>
    where
        P: AsRef<Path>,
    {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            match extattr::lgetxattr(path.as_ref(), crate::defs::REPLACE_DIR_XATTR) {
                Ok(value) if value == b"y" => return Ok(true),
                Ok(_) => {}
                Err(err) if err.0 == libc::ENODATA => {}
                Err(err) => return Err(err.into()),
            }
        }

        Ok(path.as_ref().join(REPLACE_DIR_FILE_NAME).is_file())
    }

    pub fn new_root<S>(name: S) -> Self
    where
        S: AsRef<str> + Into<String>,
    {
        Self {
            name: name.into(),
            file_type: NodeFileType::Directory,
            children: BTreeMap::default(),
            module_path: None,
            replace: false,
            skip: false,
        }
    }

    pub fn new_module<S>(name: &S, entry: &DirEntry) -> Result<Self>
    where
        S: ToString,
    {
        let path = entry.path();
        let metadata = path.symlink_metadata()?;
        let file_type = if metadata.file_type().is_char_device() && metadata.rdev() == 0 {
            NodeFileType::Whiteout
        } else {
            NodeFileType::from(metadata.file_type())
        };
        let replace = file_type == NodeFileType::Directory && Self::dir_is_replace(&path)?;
        if replace {
            crate::scoped_log!(debug, "node", "replace marker: path={}", path.display());
        }
        Ok(Self {
            name: name.to_string(),
            file_type,
            children: BTreeMap::default(),
            module_path: Some(path),
            replace,
            skip: false,
        })
    }
}
