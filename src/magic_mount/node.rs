use std::{
    collections::{HashMap, hash_map::Entry},
    ffi::CString,
    fmt,
    fs::{DirEntry, FileType},
    os::unix::fs::{FileTypeExt, MetadataExt},
    path::{Path, PathBuf},
};

use anyhow::Result;
use extattr::lgetxattr;
use rustix::path::Arg;

use crate::magic_mount::{REPLACE_DIR_FILE_NAME, REPLACE_DIR_XATTR};

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub(super) enum NodeFileType {
    RegularFile,
    Directory,
    Symlink,
    Whiteout,
}

impl NodeFileType {
    pub(super) fn from_file_type(file_type: FileType) -> Option<Self> {
        if file_type.is_file() {
            Some(Self::RegularFile)
        } else if file_type.is_dir() {
            Some(Self::Directory)
        } else if file_type.is_symlink() {
            Some(Self::Symlink)
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub(super) struct Node {
    pub(super) name: String,
    pub(super) file_type: NodeFileType,
    pub(super) children: HashMap<String, Node>,
    // the module that owned this node
    pub(super) module_path: Option<PathBuf>,
    pub(super) replace: bool,
    pub(super) skip: bool,
}

impl fmt::Display for NodeFileType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Directory => write!(f, "Directory"),
            Self::RegularFile => write!(f, "RegularFile"),
            Self::Symlink => write!(f, "Symlink"),
            Self::Whiteout => write!(f, "Whiteout"),
        }
    }
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "name: {} file_type: {} children: {:?} module_path: {} replace: {} skip: {}",
            self.name,
            self.file_type,
            self.children,
            if let Some(p) = &self.module_path {
                p.to_string_lossy().to_string()
            } else {
                "None".to_string()
            },
            self.replace,
            self.skip
        )
    }
}
impl Node {
    pub(super) fn collect_module_files<T: AsRef<Path>>(&mut self, module_dir: T) -> Result<bool> {
        let dir = module_dir.as_ref();
        let mut has_file = false;
        for entry in dir.read_dir()?.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();

            let node = match self.children.entry(name.clone()) {
                Entry::Occupied(o) => Some(o.into_mut()),
                Entry::Vacant(v) => Self::new_module(&name, &entry).map(|it| v.insert(it)),
            };

            if let Some(node) = node {
                has_file |= if node.file_type == NodeFileType::Directory {
                    node.collect_module_files(dir.join(&node.name))? || node.replace
                } else {
                    true
                }
            }
        }

        Ok(has_file)
    }

    pub(super) fn dir_is_replace<P>(path: P) -> Result<bool>
    where
        P: AsRef<Path>,
    {
        if let Ok(v) = lgetxattr(&path, REPLACE_DIR_XATTR) {
            if String::from_utf8_lossy(&v) == "y" {
                return Ok(true);
            }
        }

        let c_path = CString::new(path.as_ref().as_str()?)?;
        let fd = unsafe { libc::open(c_path.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY) };

        if fd < 0 {
            return Ok(false);
        }

        let exists = unsafe {
            let replace = CString::new(REPLACE_DIR_FILE_NAME)?;
            libc::faccessat(fd, replace.as_ptr(), libc::F_OK, 0)
        };

        if exists == 0 { Ok(true) } else { Ok(false) }
    }

    pub(super) fn new_root<T: ToString>(name: T) -> Self {
        Node {
            name: name.to_string(),
            file_type: NodeFileType::Directory,
            children: Default::default(),
            module_path: None,
            replace: false,
            skip: false,
        }
    }

    pub(super) fn new_module<T: ToString>(name: T, entry: &DirEntry) -> Option<Self> {
        if let Ok(metadata) = entry.metadata() {
            let path = entry.path();
            let file_type = if metadata.file_type().is_char_device() && metadata.rdev() == 0 {
                Some(NodeFileType::Whiteout)
            } else {
                NodeFileType::from_file_type(metadata.file_type())
            };
            if let Some(file_type) = file_type {
                let mut replace = false;
                if file_type == NodeFileType::Directory {
                    if let Ok(s) = Self::dir_is_replace(&path) {
                         if s {
                            replace = true;
                         }
                    }
                }
                return Some(Node {
                    name: name.to_string(),
                    file_type,
                    children: Default::default(),
                    module_path: Some(path),
                    replace,
                    skip: false,
                });
            }
        }

        None
    }
}
