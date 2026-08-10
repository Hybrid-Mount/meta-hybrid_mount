// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{env, fs, path::PathBuf};

use anyhow::Result;

#[path = "xtask/src/build_meta_shared.rs"]
mod build_meta_shared;

fn load_cargo_config() -> Result<build_meta_shared::CargoConfig> {
    let toml = fs::read_to_string("Cargo.toml")?;
    Ok(toml::from_str(&toml)?)
}

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=Cargo.lock");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=xtask/src/build_meta_shared.rs");
    println!("cargo:rerun-if-changed=src/defs.rs");

    let data = load_cargo_config()?;

    gen_module_prop(&data)?;

    Ok(())
}

fn gen_module_prop(data: &build_meta_shared::CargoConfig) -> Result<()> {
    let package = &data.package;
    let id = package.name.replace('-', "_");
    let version_code = build_meta_shared::calculate_version_code(&package.version)?;
    let author = package.authors.join(" & ");
    let version = format!(
        "{}-{}",
        package.version,
        build_meta_shared::git_commit_count()?
    );
    let rendered_version = format!("v{}", version.trim());
    let content = build_meta_shared::render_module_prop(&build_meta_shared::ModulePropData {
        id: &id,
        name: &package.metadata.hybrid_mount.name,
        version: &rendered_version,
        version_code: &version_code,
        author: &author,
        description: &package.description,
        update_json: &package.metadata.hybrid_mount.update,
        upgrade_epoch: build_meta_shared::UPGRADE_EPOCH,
        webui_icon: true,
    });

    let out_dir = PathBuf::from(
        env::var_os("OUT_DIR")
            .ok_or_else(|| anyhow::anyhow!("OUT_DIR is not set for build script"))?,
    );
    fs::write(out_dir.join("module.prop"), content.as_bytes())?;

    println!("cargo:rustc-env=MODULE_ID={}", id);
    Ok(())
}
