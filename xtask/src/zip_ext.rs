// SPDX-License-Identifier: GPL-3.0-only

use std::{
    fs::{self, File},
    io,
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

    while let Some(next) = paths_queue.pop() {
        let mut entries = fs::read_dir(next)?.collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(fs::DirEntry::file_name);
        let mut child_directories = Vec::new();

        for entry in entries {
            let entry_path = entry.path();
            let file_options = cb_file_options(&entry_path);
            let metadata = fs::symlink_metadata(&entry_path)?;
            let relative_path = entry_path.strip_prefix(directory).map_err(|err| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "zip entry {} is outside {}: {err}",
                        entry_path.display(),
                        directory.display()
                    ),
                )
            })?;
            let relative_name = path_as_string(relative_path)?;

            if metadata.is_file() {
                let mut file = File::open(&entry_path)?;
                zip_writer.start_file(relative_name, file_options)?;
                io::copy(&mut file, &mut zip_writer)?;
            } else if metadata.is_dir() {
                zip_writer.add_directory(relative_name, file_options)?;
                child_directories.push(entry_path);
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("unsupported zip entry type: {}", entry_path.display()),
                )
                .into());
            }
        }

        child_directories.reverse();
        paths_queue.extend(child_directories);
    }

    zip_writer.finish()?;
    Ok(())
}

fn path_as_string(path: &Path) -> std::io::Result<String> {
    let mut path_str = String::new();
    for component in path.components() {
        let Component::Normal(os_str) = component else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid relative zip path: {}", path.display()),
            ));
        };
        let name = os_str.to_str().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("zip path is not valid UTF-8: {}", path.display()),
            )
        })?;
        if !path_str.is_empty() {
            path_str.push('/');
        }
        path_str.push_str(name);
    }
    Ok(path_str)
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
