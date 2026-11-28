use std::cell::RefCell;
use std::path::PathBuf;

use mlua::{Function, MultiValue, Value};

use super::{path_arg, strings};
use crate::errors::Result;
use crate::framework::extension::core::Script;

pub(in crate::framework::extension) const FUNCTIONS: &[&str] =
    &["bind_mount", "overlay_mount", "vfs_mount"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MountKind {
    Bind,
    Overlay,
    Vfs,
}

#[derive(Debug)]
pub struct HookMount {
    pub script: PathBuf,
    pub phase: HookPhase,
    pub kind: MountKind,
    pub source: PathBuf,
    pub target: PathBuf,
}

#[derive(Clone, Copy, Debug)]
pub enum HookPhase {
    Before,
    After,
}

pub(in crate::framework::extension) struct Hooks {
    script: Script,
    before: Option<Function>,
    after: Option<Function>,
}

impl Hooks {
    pub fn new(script: Script) -> mlua::Result<Self> {
        Ok(Self {
            before: script.callback("before")?,
            after: script.callback("after")?,
            script,
        })
    }

    pub fn collect(&self, phase: HookPhase, commands: &mut Vec<HookMount>) -> Result<()> {
        let callback = match phase {
            HookPhase::Before => &self.before,
            HookPhase::After => &self.after,
        };
        let Some(callback) = callback else {
            return Ok(());
        };
        let commands = RefCell::new(commands);
        let lua = &self.script.lua;
        lua.scope(|scope| {
            for (name, kind) in [
                ("bind_mount", MountKind::Bind),
                ("overlay_mount", MountKind::Overlay),
                ("vfs_mount", MountKind::Vfs),
            ] {
                let output = &commands;
                lua.globals().raw_set(
                    name,
                    scope.create_function(move |_, args: MultiValue| {
                        let [source, target] = strings(args)?;
                        let source = path_arg(&source)?;
                        let target = path_arg(&target)?;
                        if source.as_os_str().is_empty()
                            || !target.is_absolute()
                            || target.parent().is_none()
                        {
                            return Err(mlua::Error::runtime(
                                "expected a source and an absolute non-root target",
                            ));
                        }
                        output.borrow_mut().push(HookMount {
                            script: self.script.path.clone(),
                            phase,
                            kind,
                            source,
                            target,
                        });
                        Ok(())
                    })?,
                )?;
            }
            let result = callback.call::<()>(());
            for &name in FUNCTIONS {
                lua.globals().raw_set(name, Value::Nil)?;
            }
            result
        })
        .map_err(|err| self.script.error(&format!("API 1 {phase:?}"), err))
    }
}
