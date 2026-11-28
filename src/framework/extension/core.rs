use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use mlua::chunk::ChunkMode;
use mlua::{Function, Lua, LuaOptions, StdLib, Table, Value};

use super::header::{self, Header};
use super::{Extensions, v1, v2};
use crate::errors::{Error, Result};

pub(super) struct Script {
    pub path: PathBuf,
    pub lua: Lua,
}

impl Script {
    fn new(path: PathBuf, modules: &Path, header: &Header, body: &str) -> mlua::Result<Self> {
        let lua = Lua::new_with(
            StdLib::TABLE | StdLib::STRING | StdLib::MATH | StdLib::UTF8,
            LuaOptions::default(),
        )?;
        let globals = lua.globals();
        for key in ["load", "loadfile", "dofile", "rawset", "setmetatable"] {
            globals.raw_set(key, Value::Nil)?;
        }
        let constants = lua.create_table()?;
        let callbacks: &'static [&str] = match header {
            Header::Global => {
                constants.set("API", 1)?;
                &["before", "after"]
            }
            Header::Module(id) => {
                constants.set("API", 2)?;
                constants.set("MODULE", id.as_str())?;
                &["load"]
            }
        };
        constants.set("moduledir", modules.to_string_lossy().as_ref())?;
        let meta = lua.create_table()?;
        meta.set("__index", constants)?;
        meta.set("__metatable", false)?;
        meta.set(
            "__newindex",
            lua.create_function(move |_, (table, key, value): (Table, Value, Value)| {
                if let Value::String(name) = &key {
                    let name = name.to_str()?;
                    let name = name.as_ref();
                    if matches!(name, "API" | "MODULE" | "moduledir")
                        || v1::FUNCTIONS.contains(&name)
                        || v2::FUNCTIONS.contains(&name)
                        || (matches!(name, "before" | "after" | "load")
                            && !callbacks.contains(&name))
                    {
                        return Err(mlua::Error::runtime(format!("reserved Lua global {name}")));
                    }
                }
                table.raw_set(key, value)
            })?,
        )?;
        globals.set_metatable(Some(meta))?;
        lua.load(body)
            .set_name(format!("@{}", path.display()))
            .set_mode(ChunkMode::Text)
            .exec()?;
        Ok(Self { path, lua })
    }

    pub fn callback(&self, name: &str) -> mlua::Result<Option<Function>> {
        self.lua.globals().get(name)
    }

    pub fn error(&self, phase: &str, err: mlua::Error) -> Error {
        Error::msg(format!("{} ({phase}): {err}", self.path.display()))
    }
}

impl Extensions {
    pub fn load(directory: &Path, modules: &Path) -> Result<Self> {
        let mut extensions = Self::default();
        match directory.symlink_metadata() {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => {
                return Err(Error::msg(format!(
                    "Lua directory must be a real directory: {}",
                    directory.display()
                )));
            }
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(extensions),
            Err(err) => {
                return Err(Error::msg(format!(
                    "read Lua directory {}: {err}",
                    directory.display()
                )));
            }
        };
        let mut paths = Vec::new();
        for entry in directory.read_dir()? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "lua") && entry.file_type()?.is_file() {
                paths.push(path);
            }
        }
        paths.sort_unstable();
        for path in paths {
            extensions
                .load_script(&path, modules)
                .map_err(|err| Error::msg(format!("load {}: {err}", path.display())))?;
        }
        Ok(extensions)
    }

    fn load_script(&mut self, path: &Path, modules: &Path) -> mlua::Result<()> {
        let content = fs::read_to_string(path)?;
        let (header, body) = header::parse(&content)?;
        let script = Script::new(path.to_owned(), modules, &header, &body)?;
        match header {
            Header::Global => self.global.push(v1::Hooks::new(script)?),
            Header::Module(id) => self
                .modules
                .entry(id)
                .or_default()
                .push(v2::Hook::new(script)?),
        }
        Ok(())
    }
}
