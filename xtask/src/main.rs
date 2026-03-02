// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{env, fs, io::Write, path::Path, process::Command};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use fs_extra::{dir, file};
use serde::Deserialize;
use zip::{CompressionMethod, write::FileOptions};

mod zip_ext;
use crate::zip_ext::zip_create_from_directory_with_options;

#[derive(Deserialize)]
struct HybridMountMetadata {
    name: String,
    update: String,
}

#[derive(Deserialize)]
struct PackageMetadata {
    hybrid_mount: HybridMountMetadata,
}

#[derive(Deserialize)]
struct Package {
    version: String,
    description: String,
    metadata: PackageMetadata,
}

#[derive(Deserialize)]
struct CargoConfig {
    package: Package,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq)]
enum Arch {
    #[value(name = "arm64")]
    Arm64,
    #[value(name = "arm")]
    Arm,
    #[value(name = "x86_64")]
    X86_64,
}

impl Arch {
    fn target(&self) -> &'static str {
        match self {
            Arch::Arm64 => "arm64-v8a",
            Arch::Arm => "armeabi-v7a",
            Arch::X86_64 => "x86_64",
        }
    }
    fn android_abi(&self) -> &'static str {
        match self {
            Arch::Arm64 => "aarch64-linux-android",
            Arch::Arm => "armv7-linux-androideabi",
            Arch::X86_64 => "x86_64-linux-android",
        }
    }
}

#[derive(Parser)]
#[command(name = "xtask")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Build {
        #[arg(long)]
        release: bool,
        #[arg(long)]
        skip_webui: bool,
        #[arg(long, value_enum)]
        arch: Option<Arch>,
        #[arg(long)]
        ci: bool,
        #[arg(long)]
        tag: Option<String>,
    },
    Lint,
}

struct VersionInfo {
    clean_version: String,
    full_version: String,
    version_code: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Build {
            release,
            skip_webui,
            arch,
            ci,
            tag,
        } => {
            let (cargo_release, webui_release, target_archs) = if ci {
                (true, false, vec![Arch::Arm64])
            } else {
                let archs = if let Some(selected) = arch {
                    vec![selected]
                } else {
                    vec![Arch::Arm64, Arch::Arm, Arch::X86_64]
                };
                (release, release, archs)
            };

            let version_info = if let Some(tag_name) = tag {
                resolve_release_version(&tag_name)?
            } else {
                resolve_local_or_ci_version()?
            };

            build_full(
                cargo_release,
                webui_release,
                skip_webui,
                target_archs,
                &version_info,
            )?;
        }
        Commands::Lint => {
            run_clippy()?;
        }
    }
    Ok(())
}

fn run_clippy() -> Result<()> {
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let status = Command::new(cargo)
        .args([
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ])
        .status()
        .context("Failed to run cargo clippy")?;

    if !status.success() {
        anyhow::bail!("Clippy found issues! Please fix them before committing.");
    }
    Ok(())
}

fn build_full(
    cargo_release: bool,
    webui_release: bool,
    skip_webui: bool,
    target_archs: Vec<Arch>,
    version_info: &VersionInfo,
) -> Result<()> {
    let output_dir = Path::new("output");
    let stage_dir = output_dir.join("staging");
    if output_dir.exists() {
        fs::remove_dir_all(output_dir)?;
    }
    fs::create_dir_all(&stage_dir)?;

    if !skip_webui {
        build_webui(&version_info.clean_version, webui_release)?;
    }

    for arch in target_archs {
        compile_core(cargo_release, arch)?;
        let bin_name = "hybrid-mount";
        let profile = if cargo_release { "release" } else { "debug" };
        let src_bin = Path::new("target")
            .join(arch.android_abi())
            .join(profile)
            .join(bin_name);
        let stage_bin_dir = stage_dir.join("binaries").join(arch.target());
        fs::create_dir_all(&stage_bin_dir)?;
        if src_bin.exists() {
            file::copy(
                &src_bin,
                stage_bin_dir.join(bin_name),
                &file::CopyOptions::new().overwrite(true),
            )?;
        }
    }

    let module_src = Path::new("module");
    let options = dir::CopyOptions::new().overwrite(true).content_only(true);
    dir::copy(module_src, &stage_dir, &options)?;

    generate_module_prop(&stage_dir, version_info)?;

    let gitignore = stage_dir.join(".gitignore");
    if gitignore.exists() {
        fs::remove_file(gitignore)?;
    }

    let zip_file = output_dir.join(format!("Hybrid-Mount-{}.zip", version_info.full_version));
    let zip_options = FileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .compression_level(Some(9));
    zip_create_from_directory_with_options(&zip_file, &stage_dir, |_| zip_options)?;

    Ok(())
}

fn generate_module_prop(stage_dir: &Path, info: &VersionInfo) -> Result<()> {
    let toml_content = fs::read_to_string("Cargo.toml")?;
    let config: CargoConfig = toml::from_str(&toml_content)?;

    let meta = config.package.metadata.hybrid_mount;

    let prop_content = format!(
        r#"id=hybrid_mount
name={}
version={}
versionCode={}
author=Hybrid Mount Developers
description={}
updateJson={}
metamodule=1
webuiIcon=launcher.png
"#,
        meta.name, info.full_version, info.version_code, config.package.description, meta.update
    );

    let prop_path = stage_dir.join("module.prop");
    let mut file = fs::File::create(prop_path)?;
    file.write_all(prop_content.as_bytes())?;

    Ok(())
}

fn build_webui(version: &str, is_release: bool) -> Result<()> {
    generate_webui_constants(version, is_release)?;
    let webui_dir = Path::new("webui");
    let pnpm = if cfg!(windows) { "pnpm.cmd" } else { "pnpm" };
    let status = Command::new(pnpm)
        .current_dir(webui_dir)
        .arg("install")
        .status()?;
    if !status.success() {
        anyhow::bail!("pnpm install failed");
    }
    let status = Command::new(pnpm)
        .current_dir(webui_dir)
        .args(["run", "build"])
        .status()?;
    if !status.success() {
        anyhow::bail!("pnpm run build failed");
    }
    Ok(())
}

fn generate_webui_constants(version: &str, is_release: bool) -> Result<()> {
    let path = Path::new("webui/src/lib/constants_gen.ts");
    let content = format!(
        r#"
export const APP_VERSION = "{version}";
export const IS_RELEASE = {is_release};
export const RUST_PATHS = {{
  CONFIG: "/data/adb/hybrid-mount/config.toml",
  MODE_CONFIG: "/data/adb/hybrid-mount/module_mode.conf",
  IMAGE_MNT: "/data/adb/hybrid-mount/mnt",
  DAEMON_STATE: "/data/adb/hybrid-mount/run/daemon_state.json",
  DAEMON_LOG: "/data/adb/hybrid-mount/daemon.log",
}} as const;
export const BUILTIN_PARTITIONS = ["system", "vendor", "product", "system_ext", "odm", "oem", "apex"] as const;
"#
    );
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    Ok(())
}

fn compile_core(release: bool, arch: Arch) -> Result<()> {
    let mut cmd = Command::new("cargo");
    cmd.args([
        "ndk",
        "--platform",
        "31",
        "-t",
        arch.target(),
        "build",
        "-Z",
        "build-std",
    ])
    .env("RUSTFLAGS", "-C default-linker-libraries");
    if release {
        cmd.arg("-r");
    }
    let mut ret = cmd.spawn()?;
    let status = ret.wait()?;
    if !status.success() {
        anyhow::bail!("Compilation failed for {}", arch.target());
    }
    Ok(())
}

fn calculate_version_code(version_str: &str) -> String {
    let parts: Vec<&str> = version_str.split('.').collect();
    if parts.len() >= 3 {
        let major: u32 = parts[0].parse().unwrap_or(0);
        let minor: u32 = parts[1].parse().unwrap_or(0);
        let patch: u32 = parts[2].parse().unwrap_or(0);
        format!("{}{:02}{:02}", major, minor, patch)
    } else {
        "0".to_string()
    }
}

fn resolve_release_version(tag: &str) -> Result<VersionInfo> {
    let clean_version = tag.trim_start_matches('v');
    update_cargo_toml_version(clean_version)?;

    let commit_count = cal_git_code()?;
    let full_version = format!("{}-{}", clean_version, commit_count);
    let version_code = calculate_version_code(clean_version);

    Ok(VersionInfo {
        clean_version: clean_version.to_string(),
        full_version,
        version_code,
    })
}

fn resolve_local_or_ci_version() -> Result<VersionInfo> {
    let toml = fs::read_to_string("Cargo.toml")?;
    let data: CargoConfig = toml::from_str(&toml)?;
    let clean_version = data.package.version;
    let commit_count = cal_git_code()?;

    let full_version = format!("{}-{}", clean_version, commit_count);
    let version_code = calculate_version_code(&clean_version);

    Ok(VersionInfo {
        clean_version,
        full_version,
        version_code,
    })
}

fn update_cargo_toml_version(version: &str) -> Result<()> {
    let content = fs::read_to_string("Cargo.toml")?;
    let mut new_lines = Vec::new();
    let mut replaced = false;

    for line in content.lines() {
        if !replaced && line.starts_with("version =") {
            new_lines.push(format!("version = \"{}\"", version));
            replaced = true;
        } else {
            new_lines.push(line.to_string());
        }
    }

    let mut file = fs::File::create("Cargo.toml")?;
    for line in new_lines {
        writeln!(file, "{}", line)?;
    }
    Ok(())
}

fn cal_git_code() -> Result<i32> {
    Ok(String::from_utf8(
        Command::new("git")
            .args(["rev-list", "--count", "HEAD"])
            .output()?
            .stdout,
    )?
    .trim()
    .parse::<i32>()?)
}
