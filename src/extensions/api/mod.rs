pub(super) mod v1;
pub(super) mod v2;

use std::path::{Component, Path, PathBuf};

use mlua::{MultiValue, Value};

pub use v1::{HookMount, HookPhase, MountKind};
pub use v2::ModuleEdits;

fn strings<const COUNT: usize>(args: MultiValue) -> mlua::Result<[String; COUNT]> {
    let values = args
        .into_iter()
        .map(|value| match value {
            Value::String(value) => Ok(value.to_str()?.to_owned()),
            _ => Err(mlua::Error::runtime("expected string argument")),
        })
        .collect::<mlua::Result<Vec<_>>>()?;
    values
        .try_into()
        .map_err(|_| mlua::Error::runtime(format!("expected {COUNT} arguments")))
}

fn path_arg(value: &str) -> mlua::Result<PathBuf> {
    let path = Path::new(value);
    if value.is_empty()
        || value.chars().any(char::is_control)
        || path.components().any(|part| part == Component::ParentDir)
    {
        return Err(mlua::Error::runtime(format!("invalid path {value:?}")));
    }
    Ok(path
        .components()
        .filter(|part| *part != Component::CurDir)
        .collect())
}
