// SPDX-License-Identifier: GPL-3.0-only

//! Private, unpredictable runtime directories for mount staging.

use std::fs;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::DirBuilderExt;

use crate::errors::{Error, Result};

const NAME_ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
const MIN_NAME_LEN: usize = 22;
const NAME_LEN_VARIANTS: usize = 9;
const CREATE_ATTEMPTS: usize = 32;

/// A private per-run directory. It is removed on drop unless explicitly kept
/// because `disable_umount` intentionally retains a mount beneath it.
#[derive(Debug)]
pub struct RuntimeTempDir {
    root: PathBuf,
    cleanup: bool,
}

impl RuntimeTempDir {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn create() -> Result<Self> {
        Self::create_in_candidates(&[Path::new("/tmp"), Path::new("/tmp/rw"), Path::new("/mnt")])
    }

    fn create_in_candidates(candidates: &[&Path]) -> Result<Self> {
        let mut failures = Vec::new();
        for base in candidates {
            if !base.is_dir() {
                failures.push(format!("{}: not a directory", base.display()));
                continue;
            }

            match create_random_dir(base) {
                Ok(root) => {
                    log::info!(
                        "runtime temporary directory created: base={}, path={}",
                        base.display(),
                        root.display()
                    );
                    return Ok(Self {
                        root,
                        cleanup: true,
                    });
                }
                Err(err) => {
                    log::warn!(
                        "runtime temporary base unavailable: base={}, error={err}",
                        base.display()
                    );
                    failures.push(format!("{}: {err}", base.display()));
                }
            }
        }

        Err(Error::msg(format!(
            "no writable runtime temporary base (/tmp, /tmp/rw, /mnt): {}",
            failures.join("; ")
        )))
    }

    pub fn path(&self) -> &Path {
        &self.root
    }

    pub fn allocate_dir(&self) -> Result<PathBuf> {
        create_random_dir(&self.root)
    }

    pub fn keep(&mut self) {
        self.cleanup = false;
    }

    pub fn cleanup(mut self) -> Result<()> {
        self.remove_now()
    }

    fn remove_now(&mut self) -> Result<()> {
        if !self.cleanup {
            return Ok(());
        }

        match fs::remove_dir_all(&self.root) {
            Ok(()) => {
                self.cleanup = false;
                log::info!(
                    "runtime temporary directory removed: path={}",
                    self.root.display()
                );
                Ok(())
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                self.cleanup = false;
                Ok(())
            }
            Err(err) => Err(Error::msg(format!(
                "remove runtime temporary directory {}: {err}",
                self.root.display()
            ))),
        }
    }
}

impl Drop for RuntimeTempDir {
    fn drop(&mut self) {
        if !self.cleanup {
            log::info!(
                "runtime temporary directory retained: path={}, reason=disable_umount",
                self.root.display()
            );
            return;
        }

        if let Err(err) = self.remove_now() {
            log::warn!(
                "runtime temporary directory cleanup failed: path={}, error={err}",
                self.root.display()
            );
        }
    }
}

/// Create a new child directory with a variable-length random name and mode
/// 0700. `create_dir` provides the create-new collision/symlink guarantee.
pub fn create_random_dir(parent: &Path) -> Result<PathBuf> {
    let mut last_error = None;
    for _ in 0..CREATE_ATTEMPTS {
        let name = random_name()?;
        let path = parent.join(name);
        match create_private_dir(&path) {
            Ok(()) => return Ok(path),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                last_error = Some(err);
            }
            Err(err) => return Err(err.into()),
        }
    }

    Err(Error::msg(format!(
        "could not allocate a collision-free temporary directory under {}: {}",
        parent.display(),
        last_error
            .map(|err| err.to_string())
            .unwrap_or_else(|| "random-name allocation exhausted".to_owned())
    )))
}

#[cfg(unix)]
fn create_private_dir(path: &Path) -> std::io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700);
    builder.create(path)
}

#[cfg(not(unix))]
fn create_private_dir(path: &Path) -> std::io::Result<()> {
    fs::create_dir(path)
}

fn random_name() -> Result<String> {
    let mut random = [0u8; 31];
    getrandom::fill(&mut random)
        .map_err(|err| Error::msg(format!("getrandom for temporary directory: {err}")))?;

    let len = MIN_NAME_LEN + usize::from(random[0]) % NAME_LEN_VARIANTS;
    let mut name = String::with_capacity(len);
    for byte in &random[1..=len] {
        name.push(NAME_ALPHABET[usize::from(*byte) % NAME_ALPHABET.len()] as char);
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_names_have_no_project_or_time_signature() {
        let first = random_name().unwrap();
        let second = random_name().unwrap();

        assert_ne!(first, second);
        assert!((MIN_NAME_LEN..MIN_NAME_LEN + NAME_LEN_VARIANTS).contains(&first.len()));
        assert!(first.bytes().all(|byte| NAME_ALPHABET.contains(&byte)));
        assert!(!first.to_ascii_lowercase().contains("hybrid"));
    }

    #[test]
    fn candidates_fall_back_after_missing_base() {
        let root = std::env::temp_dir().join(format!("temp-fallback-{}", std::process::id()));
        let missing = root.join("missing");
        let fallback = root.join("fallback");
        fs::create_dir_all(&fallback).unwrap();

        let session = RuntimeTempDir::create_in_candidates(&[&missing, &fallback]).unwrap();
        assert!(session.path().starts_with(&fallback));
        assert!(session.path().is_dir());

        drop(session);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn allocated_children_are_private_and_cleaned_with_session() {
        let base = std::env::temp_dir().join(format!("temp-session-{}", std::process::id()));
        fs::create_dir_all(&base).unwrap();
        let session = RuntimeTempDir::create_in_candidates(&[&base]).unwrap();
        let root = session.path().to_path_buf();
        let child = session.allocate_dir().unwrap();

        assert!(child.starts_with(&root));
        assert!(child.is_dir());
        drop(session);
        assert!(!root.exists());

        fs::remove_dir_all(&base).ok();
    }
}
