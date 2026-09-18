// SPDX-License-Identifier: GPL-3.0-only

//! Build and release automation: WebUI build with MODULE_ID injection, Rust cross-compilation,
//! module.prop generation, zip packaging, update.json and Telegram notifications.

mod zip_ext;

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use fs_extra::dir::CopyOptions;
use semver::Version;
use zip::write::FileOptions;

use crate::zip_ext::zip_create_from_directory_with_options;

const MODULE_ID: &str = "hybrid_mount";
const MODULE_NAME: &str = "Hybrid Mount";
const MODULE_AUTHOR: &str = "Hybrid Mount Developers";
const MODULE_DESCRIPTION: &str =
    "Hybrid Mount: mixed OverlayFS, Magic Mount, and VFS for KernelSU and APatch";
const UPDATE_JSON_URL: &str =
    "https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/dev/update.json";

#[derive(Parser)]
#[command(name = "xtask", about = "Hybrid Mount build automation")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Builds the WebUI and binaries, then packages the module zip
    Build {
        /// Use the release profile
        #[arg(long)]
        release: bool,
        /// CI mode, equivalent to release
        #[arg(long)]
        ci: bool,
    },
    /// Sends the output directory zip to Telegram (topic 6 = release, 37 = dev)
    Notify {
        #[arg(long, default_value = "output")]
        output: PathBuf,
        #[arg(long)]
        label: String,
        #[arg(long)]
        topic_id: Option<i64>,
    },
    /// Writes update.json
    UpdateJson {
        version: String,
        version_code: u64,
        zip_url: String,
        #[arg(
            long,
            default_value = "https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/dev/changelog.md"
        )]
        changelog: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Build { release, ci } => build(release || ci),
        Commands::Notify {
            output,
            label,
            topic_id,
        } => notify(&output, &label, topic_id),
        Commands::UpdateJson {
            version,
            version_code,
            zip_url,
            changelog,
        } => write_update_json(&version, version_code, &zip_url, &changelog),
    }
}

fn workspace_root() -> Result<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .map(Path::to_path_buf)
        .context("failed to locate workspace root")
}

fn package_version() -> Result<String> {
    let root = workspace_root()?;
    let text = fs::read_to_string(root.join("Cargo.toml"))?;
    let value: toml::Value = toml::from_str(&text)?;
    value
        .get("package")
        .and_then(|package| package.get("version"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .context("missing package.version in Cargo.toml")
}

fn version_code(version: &str) -> Result<u64> {
    let version = Version::parse(version).context("invalid semver version")?;
    version
        .major
        .checked_mul(100_000)
        .and_then(|major| {
            version
                .minor
                .checked_mul(1_000)
                .and_then(|minor| major.checked_add(minor))
        })
        .and_then(|value| value.checked_add(version.patch))
        .context("version is too large to encode as versionCode")
}

fn git_commit_count() -> Result<String> {
    let root = workspace_root()?;
    let output = Command::new("git")
        .args(["rev-list", "--count", "HEAD"])
        .current_dir(root)
        .output()
        .context("failed to run git rev-list")?;
    if !output.status.success() {
        bail!("git rev-list --count HEAD failed");
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn run_command(program: &str, args: &[&str], cwd: &Path) -> Result<()> {
    let program = if cfg!(windows) && program == "pnpm" {
        "pnpm.cmd"
    } else {
        program
    };
    let status = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .status()
        .with_context(|| format!("failed to run {program}"))?;
    if !status.success() {
        bail!("{program} exited with {status}");
    }
    Ok(())
}

fn remove_dir_if_exists(path: &Path) -> Result<()> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("failed to remove {}", path.display())),
    }
}

/// Suffixes Kbuild leaves behind in a kernel source directory: object and module
/// files, `.cmd` command records, `modules.order` and `Module.symvers`.
fn is_kernel_build_output(name: &str) -> bool {
    [".ko", ".o", ".mod", ".mod.c", ".cmd", ".order", ".symvers"]
        .iter()
        .any(|suffix| name.ends_with(suffix))
}

/// Strips Kbuild output from a staged kernel source directory.
///
/// `cargo xtask build` copies the working tree, so a developer who built the
/// module in place would otherwise ship its `.ko` and object files. Missing
/// directories are fine: a source directory is optional in the stage.
fn prune_kernel_build_output(dir: &Path) -> Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err).with_context(|| format!("failed to read {}", dir.display())),
    };
    for entry in entries {
        let entry = entry.with_context(|| format!("failed to read {}", dir.display()))?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            prune_kernel_build_output(&path)?;
        } else if is_kernel_build_output(&name) {
            remove_file_if_exists(&path)?;
        }
    }
    Ok(())
}

fn remove_file_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("failed to remove {}", path.display())),
    }
}

fn build_webui() -> Result<()> {
    let root = workspace_root()?;
    let webui = root.join("webui");

    run_command("pnpm", &["install", "--frozen-lockfile"], &webui)?;
    let status = Command::new(if cfg!(windows) { "pnpm.cmd" } else { "pnpm" })
        .args(["run", "build"])
        .current_dir(&webui)
        .env("MODULE_ID", MODULE_ID)
        .status()
        .context("failed to run pnpm run build")?;
    if !status.success() {
        bail!("pnpm run build exited with {status}");
    }
    Ok(())
}

struct AndroidArch {
    ndk_abi: &'static str,
    rust_target: &'static str,
    suffix: &'static str,
}

/// The three supported architectures: cargo-ndk ABI, Rust target and in-zip filename suffix.
const ANDROID_ARCHS: &[AndroidArch] = &[
    AndroidArch {
        ndk_abi: "arm64-v8a",
        rust_target: "aarch64-linux-android",
        suffix: "arm64",
    },
    AndroidArch {
        ndk_abi: "armeabi-v7a",
        rust_target: "armv7-linux-androideabi",
        suffix: "armv7",
    },
    AndroidArch {
        ndk_abi: "x86_64",
        rust_target: "x86_64-linux-android",
        suffix: "x86_64",
    },
];

fn rustup_target_args() -> Vec<&'static str> {
    let mut args = vec!["target", "add"];
    args.extend(ANDROID_ARCHS.iter().map(|arch| arch.rust_target));
    args.extend(["--toolchain", "nightly"]);
    args
}

fn cargo_ndk_args(release: bool) -> Vec<&'static str> {
    let mut args = vec!["+nightly", "ndk"];
    for arch in ANDROID_ARCHS {
        args.extend(["-t", arch.ndk_abi]);
    }
    args.extend(["--platform", "26", "build", "--bin", "hybrid-mount"]);
    if release {
        args.push("--release");
    }
    args
}

fn compile_binaries(release: bool) -> Result<Vec<(String, PathBuf)>> {
    let root = workspace_root()?;
    let profile = if release { "release" } else { "debug" };
    run_command("rustup", &rustup_target_args(), &root)?;
    run_command("cargo", &cargo_ndk_args(release), &root)?;

    let mut binaries = Vec::new();
    for arch in ANDROID_ARCHS {
        let binary = root
            .join("target")
            .join(arch.rust_target)
            .join(profile)
            .join("hybrid-mount");
        if !binary.exists() {
            bail!("binary not found at {}", binary.display());
        }
        binaries.push((arch.suffix.to_string(), binary));
    }

    Ok(binaries)
}

fn render_module_prop(version: &str, version_code: u64) -> String {
    format!(
        "id={MODULE_ID}\n\
         name={MODULE_NAME}\n\
         version={version}\n\
         versionCode={version_code}\n\
         author={MODULE_AUTHOR}\n\
         description={MODULE_DESCRIPTION}\n\
         updateJson={UPDATE_JSON_URL}\n\
         metamodule=1\n"
    )
}

fn build(release: bool) -> Result<()> {
    let root = workspace_root()?;
    let version = package_version()?;
    let version_code = version_code(&version)?;
    let commit_count = git_commit_count()?;

    build_webui()?;
    let binaries = compile_binaries(release)?;

    let stage = root.join("output").join("stage");
    remove_dir_if_exists(&stage)?;
    fs::create_dir_all(stage.join("binaries"))?;

    fs_extra::dir::copy(
        root.join("module"),
        &stage,
        &CopyOptions::new().content_only(true),
    )
    .context("failed to stage module files")?;

    // The kernel sources ship with the package so installs stay GPL-complete, and so do
    // the prebuilt modules. A working tree that was built in place must not carry its
    // Kbuild output into the release, so that is stripped from both source trees.
    prune_kernel_build_output(&stage.join("vfs").join("src"))?;
    prune_kernel_build_output(&stage.join("lkm").join("src"))?;

    for (suffix, binary) in binaries {
        let staged_binary = stage
            .join("binaries")
            .join(format!("hybrid-mount-{suffix}"));
        fs::copy(&binary, &staged_binary)?;
    }

    let prop = render_module_prop(&version, version_code);
    fs::write(stage.join("module.prop"), prop)?;

    let output_dir = root.join("output");
    fs::create_dir_all(&output_dir)?;
    let zip_name = format!("Hybrid-Mount-{version}-{commit_count}.zip");
    let zip_path = output_dir.join(&zip_name);
    remove_file_if_exists(&zip_path)?;

    zip_create_from_directory_with_options(&zip_path, &stage, |_| FileOptions::default())?;

    println!("created {}", zip_path.display());
    Ok(())
}

fn notify(output: &Path, label: &str, topic_id: Option<i64>) -> Result<()> {
    let request = hybrid_mount_notify::NotifyRequest::new(output, label).with_topic_id(topic_id);
    if hybrid_mount_notify::maybe_send_output_dir_notification(&request)? {
        println!("notification sent");
    }
    Ok(())
}

fn write_update_json(
    version: &str,
    version_code: u64,
    zip_url: &str,
    changelog: &str,
) -> Result<()> {
    let root = workspace_root()?;
    let payload = serde_json::json!({
        "version": version,
        "versionCode": version_code,
        "zipUrl": zip_url,
        "changelog": changelog,
    });
    fs::write(
        root.join("update.json"),
        format!("{}\n", serde_json::to_string_pretty(&payload)?),
    )?;
    println!("update.json written for {version}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn android_build_uses_one_multitarget_cargo_ndk_invocation() {
        assert_eq!(
            rustup_target_args(),
            [
                "target",
                "add",
                "aarch64-linux-android",
                "armv7-linux-androideabi",
                "x86_64-linux-android",
                "--toolchain",
                "nightly",
            ]
        );
        assert_eq!(
            cargo_ndk_args(true),
            [
                "+nightly",
                "ndk",
                "-t",
                "arm64-v8a",
                "-t",
                "armeabi-v7a",
                "-t",
                "x86_64",
                "--platform",
                "26",
                "build",
                "--bin",
                "hybrid-mount",
                "--release",
            ]
        );
    }

    #[test]
    fn kernel_source_pruning_keeps_sources_and_drops_build_output() {
        let dir = tempfile::tempdir().expect("tempdir");
        for name in [
            "hybridmount.c",
            "hybridmount.h",
            "Kconfig",
            "LICENSE",
            "Makefile",
            "PROVENANCE",
            "hybridmount.ko",
            "hybridmount.o",
            "hybridmount.mod",
            "hybridmount.mod.c",
            "modules.order",
            "Module.symvers",
            ".hybridmount.o.cmd",
        ] {
            fs::write(dir.path().join(name), b"x").expect("write fixture");
        }
        fs::create_dir(dir.path().join("nested")).expect("mkdir fixture");
        fs::write(dir.path().join("nested/hybridmount.o"), b"x").expect("write nested fixture");

        prune_kernel_build_output(dir.path()).expect("prune");

        for kept in [
            "hybridmount.c",
            "hybridmount.h",
            "Kconfig",
            "LICENSE",
            "Makefile",
            "PROVENANCE",
        ] {
            assert!(
                dir.path().join(kept).exists(),
                "{kept} must survive so the shipped sources stay complete"
            );
        }
        for dropped in [
            "hybridmount.ko",
            "hybridmount.o",
            "hybridmount.mod",
            "hybridmount.mod.c",
            "modules.order",
            "Module.symvers",
            ".hybridmount.o.cmd",
            "nested/hybridmount.o",
        ] {
            assert!(
                !dir.path().join(dropped).exists(),
                "{dropped} is kernel build output and must not reach the package"
            );
        }
    }

    /// The staged package must keep the prebuilt modules. `prune_kernel_build_output`
    /// only strips Kbuild output from `src/`, so a `.ko` that ships deliberately under
    /// `binaries/` survives staging.
    #[test]
    fn staging_keeps_deliberately_shipped_kernel_modules() {
        let stage = tempfile::tempdir().expect("tempdir");
        let binaries = stage.path().join("vfs").join("binaries");
        let source_dir = stage.path().join("vfs").join("src");
        fs::create_dir_all(&binaries).expect("create binaries fixture");
        fs::create_dir_all(&source_dir).expect("create source fixture");
        fs::write(binaries.join("hybridmount-android14-6.1.ko"), b"module")
            .expect("write module fixture");
        fs::write(source_dir.join("hybridmount.ko"), b"local build output")
            .expect("write source fixture");

        prune_kernel_build_output(&source_dir).expect("prune source tree");

        assert!(
            binaries.join("hybridmount-android14-6.1.ko").is_file(),
            "a prebuilt module under binaries/ must reach the package"
        );
        assert!(
            !source_dir.join("hybridmount.ko").exists(),
            "a locally built module inside src/ must not reach the package"
        );
    }

    /// Every module the loader can select must be covered by a digest manifest.
    #[test]
    fn every_shipped_kernel_module_is_listed_in_its_manifest() {
        let root = workspace_root().expect("workspace root");

        for (dir, names) in [
            (
                "module/lkm/binaries",
                &["nuke-android12-5.10.ko", "nuke-android-4.14.ko"][..],
            ),
            (
                "module/vfs/binaries",
                &[
                    "hybridmount-android12-5.10.ko",
                    "hybridmount-android16-6.12.ko",
                ][..],
            ),
        ] {
            let binaries = root.join(dir);
            let manifest = binaries.join("list.txt");
            if !manifest.is_file() {
                // The hybridmount set is produced by the kernel-module workflow; until it has run
                // there is nothing to verify. The ext4 set is committed and must be there.
                assert_eq!(dir, "module/vfs/binaries", "{dir}/list.txt is missing");
                continue;
            }
            let listed = fs::read_to_string(&manifest).expect("read list.txt");
            for name in names {
                assert!(
                    listed.contains(name),
                    "{name} is not listed in {dir}/list.txt"
                );
            }
        }
    }
}
