// SPDX-License-Identifier: GPL-3.0-only

use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

use zip::{ZipWriter, result::ZipResult, write::FileOptions};

/// 把目录递归打包为 zip;每个条目通过回调决定 FileOptions。
pub fn zip_create_from_directory_with_options<F>(
    archive_file: &Path,
    directory: &Path,
    cb_file_options: F,
) -> ZipResult<()>
where
    F: Fn(&PathBuf) -> FileOptions<()>,
{
    let file = File::create(archive_file)?;
    let zip_writer = ZipWriter::new(file);
    create_from_directory_with_options(zip_writer, directory, cb_file_options)
}

fn create_from_directory_with_options<F>(
    mut zip_writer: ZipWriter<File>,
    directory: &Path,
    cb_file_options: F,
) -> ZipResult<()>
where
    F: Fn(&PathBuf) -> FileOptions<()>,
{
    let mut paths_queue = vec![directory.to_path_buf()];
    let mut buffer = Vec::new();

    while let Some(next) = paths_queue.pop() {
        let entries = fs::read_dir(next)?;
        for entry in entries {
            let entry = entry?;
            let entry_path = entry.path();
            let file_options = cb_file_options(&entry_path);
            let metadata = fs::metadata(&entry_path)?;

            if metadata.is_file() {
                let mut file = File::open(&entry_path)?;
                file.read_to_end(&mut buffer)?;
                let relative_path = make_relative_path(directory, &entry_path);
                zip_writer.start_file(path_as_string(&relative_path), file_options)?;
                zip_writer.write_all(&buffer)?;
                buffer.clear();
            } else if metadata.is_dir() {
                let relative_path = make_relative_path(directory, &entry_path);
                zip_writer.add_directory(path_as_string(&relative_path), file_options)?;
                paths_queue.push(entry_path);
            }
        }
    }

    zip_writer.finish()?;
    Ok(())
}

fn make_relative_path(root: &Path, current: &Path) -> PathBuf {
    let root_components = root.components().collect::<Vec<Component>>();
    let current_components = current.components().collect::<Vec<_>>();
    let mut result = PathBuf::new();

    for (index, current_component) in current_components.iter().enumerate() {
        if index < root_components.len() {
            if root_components[index] != *current_component {
                break;
            }
        } else {
            result.push(current_component);
        }
    }

    result
}

fn path_as_string(path: &Path) -> String {
    let mut path_str = String::new();
    for component in path.components() {
        if let Component::Normal(os_str) = component {
            if !path_str.is_empty() {
                path_str.push('/');
            }
            path_str.push_str(&os_str.to_string_lossy());
        }
    }
    path_str
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use zip::ZipArchive;

    #[test]
    fn zip_contains_relative_entries_without_root() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("a.txt"), "a").unwrap();
        fs::write(dir.path().join("sub/b.txt"), "b").unwrap();

        let archive = dir.path().join("out.zip");
        zip_create_from_directory_with_options(&archive, dir.path(), |_| FileOptions::default())
            .unwrap();

        let file = File::open(&archive).unwrap();
        let mut zip = ZipArchive::new(file).unwrap();
        let names: Vec<String> = (0..zip.len())
            .map(|index| zip.by_index(index).unwrap().name().to_string())
            .collect();

        assert!(names.contains(&"a.txt".to_string()));
        assert!(names.contains(&"sub/".to_string()));
        assert!(names.contains(&"sub/b.txt".to_string()));
    }
}
