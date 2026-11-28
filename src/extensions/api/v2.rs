use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use mlua::{Function, MultiValue, Value};
use serde::{Deserialize, Serialize};

use super::{path_arg, strings};
use crate::config::Mode;
use crate::errors::Result;
use crate::extensions::core::Script;
use crate::module_id::ModuleId;
use crate::partitions::PartitionLayout;

pub(in crate::extensions) const FUNCTIONS: &[&str] =
    &["skip", "block", "remove", "replace", "change_mount"];

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct ModuleEdits {
    pub skip: bool,
    pub blocked: BTreeSet<PathBuf>,
    pub removed: BTreeSet<PathBuf>,
    pub replacements: BTreeMap<PathBuf, PathBuf>,
    pub modes: BTreeMap<PathBuf, Mode>,
}

impl ModuleEdits {
    pub fn replacement_root<'a>(&self, path: &'a Path) -> Option<&'a Path> {
        if self.replacements.is_empty() {
            return None;
        }
        path.ancestors()
            .find(|parent| self.replacements.contains_key(*parent))
    }

    pub fn excludes(&self, path: &Path) -> bool {
        (!self.blocked.is_empty() || !self.removed.is_empty())
            && path
                .ancestors()
                .any(|ancestor| self.blocked.contains(ancestor) || self.removed.contains(ancestor))
    }

    pub fn source(&self, root: &Path, path: &Path) -> PathBuf {
        if let Some(ancestor) = self.replacement_root(path)
            && let Ok(suffix) = path.strip_prefix(ancestor)
        {
            let source = &self.replacements[ancestor];
            return if suffix.as_os_str().is_empty() {
                source.clone()
            } else {
                source.join(suffix)
            };
        }
        root.join(path)
    }
}

fn relative(text: &str, layout: &PartitionLayout) -> mlua::Result<PathBuf> {
    let path = path_arg(text)?;
    let Some(Component::Normal(partition)) = path.components().next() else {
        return Err(mlua::Error::runtime("expected module-relative path"));
    };
    let name = partition.to_string_lossy();
    if name != "system"
        && !layout.roots.contains_key(name.as_ref())
        && !layout.errors.contains_key(name.as_ref())
    {
        return Err(mlua::Error::runtime(format!(
            "unknown mount partition {name}"
        )));
    }
    if text.contains(['\\', ':']) {
        return Err(mlua::Error::runtime(format!(
            "invalid module path {text:?}"
        )));
    }
    Ok(path)
}

pub(in crate::extensions) struct Hook {
    script: Script,
    load: Function,
}

impl Hook {
    pub fn new(script: Script) -> mlua::Result<Self> {
        let load = script
            .callback("load")?
            .ok_or_else(|| mlua::Error::runtime("API 2 requires load()"))?;
        let info = load.info();
        if info.what != "Lua" || info.num_params != 0 || info.is_vararg {
            return Err(mlua::Error::runtime("expected load() with no parameters"));
        }
        Ok(Self { script, load })
    }

    pub fn collect(
        &self,
        id: &ModuleId,
        layout: &PartitionLayout,
        edits: &mut ModuleEdits,
    ) -> Result<()> {
        let lua = &self.script.lua;
        let edits = RefCell::new(edits);
        lua.scope(|scope| {
            for &name in FUNCTIONS {
                let edits = &edits;
                lua.globals().raw_set(
                    name,
                    scope.create_function(move |_, args: MultiValue| {
                        let mut edits = edits.borrow_mut();
                        if name == "skip" {
                            if !args.is_empty() {
                                return Err(mlua::Error::runtime("skip expects no arguments"));
                            }
                            **edits = ModuleEdits {
                                skip: true,
                                ..ModuleEdits::default()
                            };
                        } else if !edits.skip {
                            match name {
                                "block" | "remove" => {
                                    let [path] = strings(args)?;
                                    let path = relative(&path, layout)?;
                                    if name == "block" {
                                        edits.blocked.insert(path);
                                    } else {
                                        if path.components().count()
                                            <= layout.root_len(&path.to_string_lossy())
                                        {
                                            return Err(mlua::Error::runtime(
                                                "cannot remove a partition root",
                                            ));
                                        }
                                        edits.removed.insert(path);
                                    }
                                }
                                "replace" => {
                                    let [source, target] = strings(args)?;
                                    let source = path_arg(&source)?;
                                    if !source.is_absolute() {
                                        return Err(mlua::Error::runtime(
                                            "replace source must be absolute",
                                        ));
                                    }
                                    edits
                                        .replacements
                                        .insert(relative(&target, layout)?, source);
                                }
                                "change_mount" => {
                                    let [path, mode] = strings(args)?;
                                    let mode = match mode.as_str() {
                                        "vfs" => Mode::Vfs,
                                        "overlay" => Mode::Overlay,
                                        "magic" => Mode::Magic,
                                        _ => {
                                            return Err(mlua::Error::runtime(
                                                "change_mount expects vfs, overlay or magic",
                                            ));
                                        }
                                    };
                                    edits.modes.insert(relative(&path, layout)?, mode);
                                }
                                _ => unreachable!(),
                            }
                        }
                        Ok(())
                    })?,
                )?;
            }
            let result = self.load.call::<()>(());
            for &name in FUNCTIONS {
                lua.globals().raw_set(name, Value::Nil)?;
            }
            result
        })
        .map_err(|err| self.script.error(&format!("API 2 load for {id}"), err))
    }
}
