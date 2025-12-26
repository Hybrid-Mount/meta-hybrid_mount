// Copyright 2025 Meta-Hybrid Mount Authors
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::HashMap,
    fmt,
    fs::FileType,
    path::{Component, PathBuf},
};

use crate::core::modules::ModuleFile;

#[derive(PartialEq, Eq, Hash, Clone, Debug, Copy)]

pub enum NodeFileType {
    RegularFile,
    Directory,
    Symlink,
    Whiteout,
}

impl From<FileType> for NodeFileType {
    fn from(file_type: FileType) -> Self {
        if file_type.is_dir() {
            Self::Directory
        } else if file_type.is_symlink() {
            Self::Symlink
        } else {
            Self::RegularFile
        }
    }
}

impl fmt::Display for NodeFileType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Directory => write!(f, "DIR"),
            Self::RegularFile => write!(f, "FILE"),
            Self::Symlink => write!(f, "LINK"),
            Self::Whiteout => write!(f, "WHT"),
        }
    }
}

#[derive(Clone)]

pub struct Node {
    pub name: String,
    pub file_type: NodeFileType,
    pub children: HashMap<String, Self>,
    pub module_path: Option<PathBuf>,
    pub replace: bool,
    pub skip: bool,
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

            let source_str = if let Some(p) = &node.module_path {
                format!(" -> {}", p.display())
            } else {
                String::new()
            };

            writeln!(
                f,
                "{}{}{} [{}]{}{}",
                prefix, connector, name, node.file_type, flag_str, source_str
            )?;

            let child_prefix = if is_root {
                ""
            } else if is_last {
                "    "
            } else {
                "│   "
            };

            let new_prefix = format!("{}{}", prefix, child_prefix);

            let mut children: Vec<_> = node.children.values().collect();

            children.sort_by(|a, b| a.name.cmp(&b.name));

            for (i, child) in children.iter().enumerate() {
                let is_last_child = i == children.len() - 1;

                print_tree(child, f, &new_prefix, is_last_child, false)?;
            }

            Ok(())
        }

        print_tree(self, f, "", true, true)
    }
}

impl Node {
    pub fn new_root<S>(name: S) -> Self
    where
        S: Into<String>,
    {
        Self {
            name: name.into(),
            file_type: NodeFileType::Directory,
            module_path: None,
            children: HashMap::new(),
            replace: false,
            skip: false,
        }
    }

    pub fn collect_module_files(&mut self, root: &PathBuf) -> anyhow::Result<()> {
        for entry in walkdir::WalkDir::new(root)
            .min_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            let relative_path = path.strip_prefix(root)?;

            let module_file = ModuleFile::new(root, relative_path)?;

            if module_file.is_replace_file {
                continue;
            }

            self.add_module_file(module_file);
        }

        Ok(())
    }

    fn add_module_file(&mut self, module_file: ModuleFile) {
        let mut current_node = self;

        let components: Vec<Component> = module_file.relative_path.components().collect();

        for (i, component) in components.iter().enumerate() {
            let name = component.as_os_str().to_string_lossy().to_string();

            let is_last = i == components.len() - 1;

            if is_last {
                let file_type = if module_file.is_whiteout {
                    NodeFileType::Whiteout
                } else {
                    NodeFileType::from(module_file.file_type)
                };

                let node = current_node
                    .children
                    .entry(name.clone())
                    .or_insert_with(|| Node {
                        name: name.clone(),
                        file_type,
                        module_path: None,
                        children: HashMap::new(),
                        replace: false,
                        skip: false,
                    });

                if !module_file.is_whiteout {
                    node.module_path = Some(module_file.real_path.clone());

                    node.file_type = file_type;
                } else {
                    node.file_type = NodeFileType::Whiteout;
                }

                if module_file.is_replace {
                    node.replace = true;
                }
            } else {
                current_node = current_node
                    .children
                    .entry(name.clone())
                    .or_insert_with(|| Node {
                        name,
                        file_type: NodeFileType::Directory,
                        module_path: None,
                        children: HashMap::new(),
                        replace: false,
                        skip: false,
                    });
            }
        }
    }
}
