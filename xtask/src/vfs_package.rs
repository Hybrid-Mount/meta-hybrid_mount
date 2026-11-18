// SPDX-License-Identifier: GPL-3.0-only

use anyhow::{Context, Result, ensure};
use ring::digest::{SHA256, digest};
use std::{collections::BTreeSet, fmt::Write, fs, path::Path};

use crate::lkm_packages::TARGETS;

fn digest_hex(bytes: &[u8]) -> Result<String> {
    let mut result = String::with_capacity(64);
    for byte in digest(&SHA256, bytes).as_ref() {
        write!(result, "{byte:02x}")?;
    }
    Ok(result)
}

fn checksum_line(dir: &Path, name: &str) -> Result<String> {
    let path = dir.join(name);
    let bytes = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    Ok(format!("{}  {name}", digest_hex(&bytes)?))
}

pub(super) fn verify(root: &Path) -> Result<()> {
    let source = root.join("module/vfs/src");
    let binaries = root.join("module/vfs/binaries");
    let mut input = String::new();
    for name in ["hybridmount.c", "hybridmount.h", "Kconfig", "Makefile"] {
        writeln!(input, "{}", checksum_line(&source, name)?)?;
    }
    let stamp = fs::read_to_string(binaries.join("sources.txt"))?;
    ensure!(
        stamp.trim() == digest_hex(input.as_bytes())?,
        "VFS prebuilts do not match the current kernel sources"
    );

    let manifest = fs::read_to_string(binaries.join("list.txt"))?;
    let expected = TARGETS.iter().map(|t| t.3).collect::<BTreeSet<_>>();
    let mut lines = manifest.lines().collect::<BTreeSet<_>>();
    ensure!(
        manifest.lines().count() == lines.len(),
        "duplicate VFS checksum entries"
    );
    for name in &expected {
        ensure!(
            lines.remove(checksum_line(&binaries, name)?.as_str()),
            "missing or incorrect VFS checksum for {name}"
        );
    }
    ensure!(
        lines.is_empty(),
        "unexpected entries in the VFS checksum manifest"
    );
    for entry in fs::read_dir(&binaries)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        ensure!(
            !name.ends_with(".ko") || expected.contains(name.as_ref()),
            "unexpected VFS module {name}"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn shipped_vfs_modules_match_sources_loader_and_manifest() {
        super::verify(&crate::workspace_root().unwrap()).unwrap();
    }

    #[test]
    fn packaging_rejects_a_missing_or_corrupted_target() {
        let root = crate::workspace_root().unwrap();
        let fixture = tempfile::tempdir().unwrap();
        let module = fixture.path().join("module");
        std::fs::create_dir(&module).unwrap();
        fs_extra::dir::copy(root.join("module/vfs"), &module, &Default::default()).unwrap();
        let target = module.join("vfs/binaries/hybridmount-android17-6.18.ko");
        std::fs::write(&target, b"invalid kernel object").unwrap();
        let error = super::verify(fixture.path()).unwrap_err();
        assert!(error.to_string().contains("incorrect VFS checksum"));
        std::fs::remove_file(&target).unwrap();
        let error = super::verify(fixture.path()).unwrap_err();
        assert!(error.to_string().contains("hybridmount-android17-6.18.ko"));
    }
}
