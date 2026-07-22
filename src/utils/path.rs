// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{
    ffi::OsStr,
    fs, io,
    path::{Component, Path, PathBuf},
};

pub fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    let mut saw_root = false;

    for component in path.components() {
        match component {
            Component::RootDir => {
                normalized.push(Path::new("/"));
                saw_root = true;
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(
                    normalized.components().next_back(),
                    Some(Component::Normal(_))
                ) {
                    normalized.pop();
                } else if !saw_root {
                    normalized.push("..");
                }
            }
            Component::Normal(value) => normalized.push(value),
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
        }
    }

    if saw_root && normalized.as_os_str().is_empty() {
        PathBuf::from("/")
    } else {
        normalized
    }
}

pub fn path_file_name_eq(path: &Path, expected: &str) -> bool {
    path.file_name()
        .is_some_and(|name| name == OsStr::new(expected))
}

pub fn resolve_link_path(path: &Path) -> io::Result<PathBuf> {
    match fs::read_link(path) {
        Ok(target) if target.is_absolute() => Ok(normalize_path(&target)),
        Ok(target) => {
            let parent = path.parent().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "symlink path has no parent")
            })?;
            Ok(normalize_path(&parent.join(target)))
        }
        Err(err) if err.kind() == io::ErrorKind::InvalidInput => Ok(normalize_path(path)),
        Err(err) => Err(err),
    }
}

#[cfg(feature = "kasumi")]
pub fn resolve_path_with_root(system_root: &Path, path: &Path) -> io::Result<PathBuf> {
    let virtual_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        Path::new("/").join(path)
    };

    let translated_path = if system_root == Path::new("/") {
        virtual_path.clone()
    } else {
        let relative = virtual_path.strip_prefix("/").map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("virtual path must be absolute: {error}"),
            )
        })?;
        system_root.join(relative)
    };

    let parent = translated_path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "translated path has no parent")
    })?;

    let filename = translated_path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "translated path has no filename",
        )
    })?;

    let mut current = parent.to_path_buf();
    let mut suffix = Vec::new();

    while current != system_root && !current.exists() {
        if let Some(name) = current.file_name() {
            suffix.push(name.to_os_string());
        }
        if !current.pop() {
            break;
        }
    }

    if !current.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no existing ancestor for {}", translated_path.display()),
        ));
    }
    let mut resolved = current;

    for item in suffix.iter().rev() {
        resolved.push(item);
    }
    resolved.push(filename);

    if system_root == Path::new("/") {
        return Ok(resolved);
    }

    let relative = resolved.strip_prefix(system_root).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "resolved path {} escaped root {}: {error}",
                resolved.display(),
                system_root.display()
            ),
        )
    })?;
    Ok(Path::new("/").join(relative))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_path_preserves_leading_relative_parents() {
        assert_eq!(
            normalize_path(Path::new("../../system/bin")),
            PathBuf::from("../../system/bin")
        );
    }

    #[test]
    fn normalize_path_clamps_absolute_parents_at_root() {
        assert_eq!(
            normalize_path(Path::new("/../../system")),
            PathBuf::from("/system")
        );
    }

    #[test]
    fn normalize_path_removes_resolved_parent_components() {
        assert_eq!(
            normalize_path(Path::new("system/../vendor")),
            PathBuf::from("vendor")
        );
    }
}
