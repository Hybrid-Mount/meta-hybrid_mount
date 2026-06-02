use std::{env, fs, path::PathBuf};

use anyhow::{Context, Result, anyhow};

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

    #[cfg(feature = "kasumi")]
    gen_kasumi_uapi_bindings()?;
    gen_module_prop(&data)?;

    Ok(())
}

#[cfg(feature = "kasumi")]
fn kasumi_uapi_header_path() -> PathBuf {
    PathBuf::from("src/sys/kasumi_uapi.h")
}

#[cfg(feature = "kasumi")]
fn gen_kasumi_uapi_bindings() -> Result<()> {
    let header = kasumi_uapi_header_path();
    let header = fs::canonicalize(&header)
        .with_context(|| format!("failed to resolve Kasumi UAPI header {}", header.display()))?;
    println!("cargo:rerun-if-changed={}", header.display());

    let out_dir = PathBuf::from(
        env::var_os("OUT_DIR").ok_or_else(|| anyhow!("OUT_DIR is not set for build script"))?,
    );
    let wrapper = out_dir.join("kasumi_uapi_wrapper.h");
    let bindings = out_dir.join("kasumi_uapi.rs");

    fs::write(
        &wrapper,
        format!(
            r#"#include <stdint.h>
#include <stddef.h>
#ifndef __u32
typedef uint32_t __u32;
#endif
#ifndef __aligned_u64
typedef uint64_t __aligned_u64;
#endif
#include "{}"
"#,
            header.display()
        ),
    )?;

    bindgen::Builder::default()
        .header(wrapper.to_string_lossy())
        .allowlist_type("kasumi_.*")
        .allowlist_var("KSM_.*")
        .derive_debug(true)
        .derive_copy(true)
        .derive_default(false)
        .layout_tests(false)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .map_err(|err| anyhow!("failed to generate Kasumi UAPI bindings: {err}"))?
        .write_to_file(&bindings)
        .with_context(|| format!("failed to write {}", bindings.display()))?;

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
        webui_icon: true,
    });

    let out_dir = PathBuf::from(
        env::var_os("OUT_DIR")
            .ok_or_else(|| anyhow::anyhow!("OUT_DIR is not set for build script"))?,
    );
    fs::write(out_dir.join("module.prop"), content.as_bytes())?;
    Ok(())
}
