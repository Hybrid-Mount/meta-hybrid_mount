// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::{
    ffi::OsStr,
    fs, io,
    path::{Component, Path, PathBuf},
};

fn os_str_eq_ignore_ascii_case(value: &OsStr, expected: &str) -> bool {
    value
        .as_encoded_bytes()
        .eq_ignore_ascii_case(expected.as_bytes())
}

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
                let _ = normalized.pop();
                if saw_root && normalized.as_os_str().is_empty() {
                    normalized.push(Path::new("/"));
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

pub fn path_file_name_eq_ignore_ascii_case(path: &Path, expected: &str) -> bool {
    path.file_name()
        .is_some_and(|name| os_str_eq_ignore_ascii_case(name, expected))
}

pub fn find_dir_entry_case_insensitive(dir: &Path, expected: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        if os_str_eq_ignore_ascii_case(&entry.file_name(), expected) {
            return Some(entry.path());
        }
    }
    None
}

pub fn dir_contains_entry_case_insensitive(dir: &Path, expected: &str) -> bool {
    find_dir_entry_case_insensitive(dir, expected).is_some()
}

pub fn remove_dir_entries_case_insensitive(dir: &Path, expected: &str) -> io::Result<usize> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(err) => return Err(err),
    };

    let mut removed = 0usize;
    for entry in entries {
        let entry = entry?;
        if os_str_eq_ignore_ascii_case(&entry.file_name(), expected) {
            fs::remove_file(entry.path())?;
            removed += 1;
        }
    }

    Ok(removed)
}

pub fn resolve_link_path(path: &Path) -> PathBuf {
    match fs::read_link(path) {
        Ok(target) if target.is_absolute() => normalize_path(&target),
        Ok(target) => normalize_path(&path.parent().unwrap_or(Path::new("/")).join(target)),
        Err(_) => normalize_path(path),
    }
}

#[cfg(feature = "kasumi")]
pub fn resolve_path_with_root(system_root: &Path, path: &Path) -> PathBuf {
    let virtual_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        Path::new("/").join(path)
    };

    let translated_path = if system_root == Path::new("/") {
        virtual_path.clone()
    } else {
        let relative = virtual_path.strip_prefix("/").unwrap_or(&virtual_path);
        system_root.join(relative)
    };

    let Some(parent) = translated_path.parent() else {
        return virtual_path;
    };

    let Some(filename) = translated_path.file_name() else {
        return virtual_path;
    };

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

    let mut resolved = if current.exists() {
        current
    } else {
        parent.to_path_buf()
    };

    for item in suffix.iter().rev() {
        resolved.push(item);
    }
    resolved.push(filename);

    if system_root == Path::new("/") {
        return resolved;
    }

    if let Ok(relative) = resolved.strip_prefix(system_root) {
        return Path::new("/").join(relative);
    }

    virtual_path
}
