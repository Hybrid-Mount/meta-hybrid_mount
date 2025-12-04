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

use crate::defs::{REPLACE_DIR_FILE_NAME, REPLACE_DIR_XATTR};

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

#[derive(Debug)]
pub struct Node {
    pub name: String,
    pub file_type: NodeFileType,
    pub children: HashMap<String, Self>,
    // the module that owned this node
    pub module_path: Option<PathBuf>,
    pub replace: bool,
    pub skip: bool,
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

fn print_tree(
    node: &Node,
    f: &mut fmt::Formatter<'_>,
    prefix: &str,
    is_last: bool,
    is_root: bool,
) -> fmt::Result {
    let connector = if is_root {
        ""
    } else if is_last {
        "└── "
    } else {
        "├── "
    };

    let name = if node.name.is_empty() {
        "/"
    } else {
        &node.name
    };

    // Collect flags
    let mut flags = Vec::new();
    if node.replace {
        flags.push("REPLACE");
    }
    if node.skip {
        flags.push("SKIP");
    }

    let flag_str = if flags.is_empty() {
        String::new()
    } else {
        format!(" [{}]", flags.join("|"))
    };

    // Source path info
    let source_str = node
        .module_path
        .as_ref()
        .map_or_else(String::new, |p| format!(" -> {}", p.display()));

    // Line format: ├── name [TYPE] [FLAGS] -> /path
    writeln!(
        f,
        "{}{}{} [{}]{}{}",
        prefix, connector, name, node.file_type, flag_str, source_str
    )?;

    // Calculate prefix for children
    let child_prefix = if is_root {
        ""
    } else if is_last {
        "    "
    } else {
        "│   "
    };
    let new_prefix = format!("{prefix}{child_prefix}");

    // Sort children by name for deterministic logs
    let mut children: Vec<_> = node.children.values().collect();
    children.sort_by(|a, b| a.name.cmp(&b.name));

    for (i, child) in children.iter().enumerate() {
        let is_last_child = i == children.len() - 1;
        print_tree(child, f, &new_prefix, is_last_child, false)?;
    }
    Ok(())
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        print_tree(self, f, "", true, true)
    }
}

impl Node {
    pub fn collect_module_files<P>(&mut self, module_dir: P) -> Result<bool>
    where
        P: AsRef<Path>,
    {
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

    pub fn dir_is_replace<P>(path: P) -> Result<bool>
    where
        P: AsRef<Path>,
    {
        if let Ok(v) = lgetxattr(&path, REPLACE_DIR_XATTR)
            && String::from_utf8_lossy(&v) == "y"
        {
            return Ok(true);
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

    pub fn new_root<S>(name: S) -> Self
    where
        S: AsRef<str> + Into<String>,
    {
        Self {
            name: name.into(),
            file_type: NodeFileType::Directory,
            children: HashMap::default(),
            module_path: None,
            replace: false,
            skip: false,
        }
    }

    pub fn new_module<S>(name: &S, entry: &DirEntry) -> Option<Self>
    where
        S: AsRef<str> + Into<String>,
        std::string::String: for<'a> From<&'a S>,
    {
        if let Ok(metadata) = entry.metadata() {
            let path = entry.path();
            let file_type = if metadata.file_type().is_char_device() && metadata.rdev() == 0 {
                Some(NodeFileType::Whiteout)
            } else {
                Some(NodeFileType::from(metadata.file_type()))
            };
            if let Some(file_type) = file_type {
                let replace = if file_type == NodeFileType::Directory
                    && let Ok(s) = Self::dir_is_replace(&path)
                    && s
                {
                    true
                } else {
                    false
                };
                return Some(Self {
                    name: name.into(),
                    file_type,
                    children: HashMap::default(),
                    module_path: Some(path),
                    replace,
                    skip: false,
                });
            }
        }

        None
    }
}
