// SPDX-License-Identifier: GPL-3.0-only

//! Boot-time compatibility with the upstream Magic Mount Lua directory hooks.

use std::fs;
use std::path::{Component, Path, PathBuf};

use mlua::chunk::ChunkMode;
use mlua::{Function, Lua, LuaOptions, StdLib, Table, Value};

use crate::errors::{Error, Result};
use crate::scanner::ModuleRecord;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) const DIRECTORY: &str = "/data/adb/magic_mount/lua";

struct Script {
    path: PathBuf,
    lua: Lua,
    modules: Table,
    before: Option<Function>,
    after: Option<Function>,
}

impl Script {
    fn load(path: &Path, modules: &[ModuleRecord], module_dir: &Path) -> mlua::Result<Self> {
        let lua = Lua::new_with(
            StdLib::TABLE | StdLib::STRING | StdLib::MATH | StdLib::UTF8,
            LuaOptions::default(),
        )?;
        for name in ["dofile", "loadfile"] {
            lua.globals().set(name, Value::Nil)?;
        }
        let info = lua.create_table()?;
        for module in modules {
            let entry = lua.create_table()?;
            entry.set("id", module.id.as_str())?;
            entry.set("name", module.name.as_str())?;
            entry.set("version", module.version.as_str())?;
            entry.set("author", module.author.as_str())?;
            entry.set("description", module.description.as_str())?;
            entry.set("enabled", !module.disabled)?;
            entry.set("source_path", module.source_path.to_string_lossy().as_ref())?;
            entry.set("skip", !module.mountable())?;
            entry.set("is_mounted", module.mountable())?;
            info.set(module.id.as_str(), entry)?;
        }
        lua.globals().set("modules", info.clone())?;
        lua.globals()
            .set("moduledir", module_dir.to_string_lossy().as_ref())?;
        let source = fs::read_to_string(path).map_err(mlua::Error::external)?;
        lua.load(&source)
            .set_name(format!("@{}", path.display()))
            .set_mode(ChunkMode::Text)
            .exec()?;
        let before = lua.globals().get("before")?;
        let after = lua.globals().get("after")?;
        Ok(Self {
            path: path.to_owned(),
            lua,
            modules: info,
            before,
            after,
        })
    }

    fn read_skips(&self, modules: &mut [ModuleRecord]) -> mlua::Result<()> {
        if let Some(before) = &self.before {
            before.call::<()>(())?;
        }
        for module in modules {
            let info: Table = self.modules.raw_get(module.id.as_str())?;
            if info.raw_get::<Value>("skip")? == Value::Boolean(true) {
                module.skip_mount = true;
            }
        }
        Ok(())
    }

    fn sync_mountability(&self, modules: &[ModuleRecord]) -> mlua::Result<()> {
        for module in modules {
            let info: Table = self.modules.raw_get(module.id.as_str())?;
            info.raw_set("skip", !module.mountable())?;
            info.raw_set("is_mounted", module.mountable())?;
        }
        Ok(())
    }

    fn queue_binds(&self, binds: &mut Vec<(String, String)>) -> mlua::Result<()> {
        let Some(after) = &self.after else {
            return Ok(());
        };
        self.lua.scope(|scope| {
            let bind = scope.create_function_mut(|_, (source, target): (String, String)| {
                if [source.as_str(), target.as_str()]
                    .iter()
                    .any(|path| path.is_empty() || path.chars().any(char::is_control))
                    || !target.starts_with('/')
                    || Path::new(&target)
                        .components()
                        .any(|part| part == Component::ParentDir)
                {
                    return Err(mlua::Error::runtime("invalid bind source or target"));
                }
                binds.push((source, target));
                Ok(())
            })?;
            self.lua.globals().set("bind", bind)?;
            let result = after.call::<()>(());
            self.lua.globals().set("bind", Value::Nil)?;
            result
        })
    }
}

pub(crate) struct Hooks(Vec<Script>);

impl Hooks {
    pub(crate) fn load(
        directory: &Path,
        modules: &mut [ModuleRecord],
        module_dir: &Path,
    ) -> Result<Self> {
        let entries = match fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self(Vec::new()));
            }
            Err(error) => {
                return Err(Error::msg(format!(
                    "read Lua directory {}: {error}",
                    directory.display()
                )));
            }
        };
        let mut paths = Vec::new();
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "lua") && entry.file_type()?.is_file() {
                paths.push(path);
            }
        }
        paths.sort_unstable();

        let scripts = paths
            .iter()
            .map(|path| {
                Script::load(path, modules, module_dir)
                    .map_err(|error| script_error(path, "load", error))
            })
            .collect::<Result<Vec<_>>>()?;
        for script in &scripts {
            script
                .read_skips(modules)
                .map_err(|error| script_error(&script.path, "before", error))?;
        }
        for script in &scripts {
            script
                .sync_mountability(modules)
                .map_err(|error| script_error(&script.path, "before", error))?;
        }
        Ok(Self(scripts))
    }

    pub(crate) fn after(&self) -> Result<Vec<(String, String)>> {
        let mut binds = Vec::new();
        for script in &self.0 {
            script
                .queue_binds(&mut binds)
                .map_err(|error| script_error(&script.path, "after", error))?;
        }
        Ok(binds)
    }
}

fn script_error(path: &Path, phase: &str, error: mlua::Error) -> Error {
    Error::msg(format!("Lua hook {} ({phase}): {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hooks_run_in_order_and_keep_separate_globals() {
        let dir = crate::test_support::Fixture::new("lua-hooks");
        fs::write(
            dir.join("20.lua"),
            r#"
            assert(private == nil and dofile == nil and loadfile == nil)
            function after() bind("/src/b", "/target/b") end
        "#,
        )
        .unwrap();
        fs::write(
            dir.join("10.lua"),
            r#"
            private = true
            function after() bind("/src/a", "/target/a") end
        "#,
        )
        .unwrap();
        let hooks = Hooks::load(&dir, &mut [], Path::new("/modules")).unwrap();
        assert_eq!(
            hooks.after().unwrap(),
            [
                ("/src/a".into(), "/target/a".into()),
                ("/src/b".into(), "/target/b".into()),
            ]
        );
    }

    #[test]
    fn before_can_skip_modules_and_after_rejects_invalid_bind_targets() {
        let dir = crate::test_support::Fixture::new("lua-skip");
        fs::create_dir_all(dir.join("modules/example/system")).unwrap();
        fs::write(
            dir.join("modules/example/module.prop"),
            "id=example\nname=Example\nversion=1\nauthor=A\ndescription=D\n",
        )
        .unwrap();
        fs::write(dir.join("modules/example/system/file"), "payload").unwrap();
        let mut modules = crate::scanner::list_modules(&dir.join("modules"), &[]).unwrap();
        assert!(modules[0].mountable());
        fs::write(
            dir.join("hook.lua"),
            r#"
            function before() modules.example.skip = true end
            function after()
                assert(modules.example.skip and not modules.example.is_mounted)
                bind("/src/a", "/target/a")
                bind("/src/b", "/target/../b")
            end
        "#,
        )
        .unwrap();
        let hooks = Hooks::load(&dir, &mut modules, &dir.join("modules")).unwrap();
        assert!(!modules[0].mountable());
        assert!(
            hooks
                .after()
                .unwrap_err()
                .to_string()
                .contains("invalid bind")
        );
    }
}
