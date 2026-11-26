// SPDX-License-Identifier: GPL-3.0-only

//! Build and release automation: WebUI build with MODULE_ID injection, Rust cross-compilation,
//! module.prop generation, zip packaging, update.json and Telegram notifications.

#[path = "../../src/vfs/lkm_packages.rs"]
mod lkm_packages;
mod vfs_package;
mod zip_ext;

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum Flavor {
    Normal,
    Wear,
}

impl Flavor {
    fn slug(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Wear => "wear",
        }
    }

    fn archs(self) -> impl Iterator<Item = &'static AndroidArch> {
        ANDROID_ARCHS.iter().filter(move |arch| {
            self == Self::Normal || matches!(arch.suffix, "arm64" | "armv7" | "x86_64")
        })
    }

    fn update_file(self) -> &'static str {
        match self {
            Self::Normal => "update.json",
            Self::Wear => "update-wear.json",
        }
    }
}

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
        /// Build only this flavor; by default build both packages
        #[arg(long, value_enum)]
        flavor: Option<Flavor>,
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
    /// Prints the version facts of a release tag as `key=value` lines
    ///
    /// The release workflow reads `version`, `version_code` and `prerelease` from here instead of
    /// deriving each one with its own shell pattern, so the version written into `Cargo.toml`, the
    /// `versionCode` published in `update.json` and the pre-release flag cannot disagree.
    ReleaseVersion {
        /// The git tag, with or without its leading `v`
        tag: String,
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
        #[arg(long, value_enum, default_value = "normal")]
        flavor: Flavor,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Build {
            release,
            ci,
            flavor,
        } => {
            for selected in flavor.map_or_else(|| vec![Flavor::Normal, Flavor::Wear], |f| vec![f]) {
                build(release || ci, selected)?;
            }
            Ok(())
        }
        Commands::Notify {
            output,
            label,
            topic_id,
        } => notify(&output, &label, topic_id),
        Commands::ReleaseVersion { tag } => print_release_version(&tag),
        Commands::UpdateJson {
            version,
            version_code,
            zip_url,
            changelog,
            flavor,
        } => write_update_json(&version, version_code, &zip_url, &changelog, flavor),
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

/// The version facts a release tag implies.
///
/// Every consumer of a tag — the injected `Cargo.toml` version, the packaged `module.prop`, the
/// published `update.json` and the GitHub pre-release flag — is derived from this one type, so a
/// tag cannot mean one version to Cargo and another to the update channel.
#[derive(Debug)]
struct ReleaseVersion {
    /// The normalised version, without the leading `v` and without build metadata
    version: String,
    /// The monotonically comparable code the module manager compares
    version_code: u64,
    /// Whether this is a release candidate rather than a finished release
    prerelease: bool,
}

impl ReleaseVersion {
    /// Parses a git tag such as `v6.2.1` or `v6.2.1-rc.1`.
    ///
    /// The tag is validated as semver before anything else, which is what the workflow used to
    /// leave to Cargo: `v6.2.01` is rejected here, with a message that names the tag, instead of
    /// surfacing later as `error: invalid leading zero in patch version number` from a build step.
    fn parse(tag: &str) -> Result<Self> {
        let trimmed = tag.trim();
        let raw = trimmed.strip_prefix('v').unwrap_or(trimmed);
        let parsed = Version::parse(raw)
            .with_context(|| format!("release tag '{trimmed}' is not a semver version"))?;

        // Build metadata does not take part in precedence, so two tags that differ only in
        // `+build` would publish the same module version. Release tags stay free of it.
        if !parsed.build.is_empty() {
            let without_build = Version {
                build: semver::BuildMetadata::EMPTY,
                ..parsed.clone()
            };
            bail!(
                "release tag '{trimmed}' carries build metadata; tag it as '{without_build}' instead"
            );
        }

        Ok(Self {
            version: parsed.to_string(),
            version_code: version_code(&parsed)?,
            prerelease: !parsed.pre.is_empty(),
        })
    }
}

/// The low-order digits reserved for ordering inside a `versionCode`.
const VERSION_STRIDE: u64 = 1_000;

/// The slot a finished release occupies: the top of its version's window, leaving every lower slot
/// to the candidates that lead to it.
const FINAL_SLOT: u64 = VERSION_STRIDE - 1;

/// The pre-release stages a `versionCode` can order, earliest first.
const CANDIDATE_STAGES: &[&str] = &["alpha", "beta", "rc"];

/// How many numbered candidates each stage may have.
const CANDIDATES_PER_STAGE: u64 = 100;

/// Encodes a version as the module manager's `versionCode`.
///
/// The published releases used `major * 100000 + minor * 1000 + patch`, which leaves no room
/// between one version and the next: a release candidate and the release it leads to could not be
/// told apart, because both would need the same code. This keeps that base and gives each version a
/// window of a thousand codes, so a candidate of `6.2.1` lands just below `6.2.1` and still above
/// `6.2.0`:
///
/// ```text
/// v6.2.0        602_000_999
/// v6.2.1-rc.1   602_001_301
/// v6.2.1        602_001_999
/// ```
///
/// The candidates have to sit *below* their release, not above it. A device is offered a version
/// only when its code is higher than the installed one, so a candidate placed above its own release
/// would look newer forever and that release would never reach anyone who had tried the candidate.
/// Every code this produces is also larger than the whole-number code the published scheme gave the
/// same version, so a device already on `6.2.0` still sees the next release as an update.
///
/// A `versionCode` is one integer, so it can only order candidates written in one shape. That shape
/// is `<stage>.<number>`, with the stage named in [`CANDIDATE_STAGES`], and anything else is
/// refused by name rather than silently given an approximate order.
fn version_code(version: &Version) -> Result<u64> {
    let base = version
        .major
        .checked_mul(100_000)
        .and_then(|major| {
            version
                .minor
                .checked_mul(1_000)
                .and_then(|minor| major.checked_add(minor))
        })
        .and_then(|value| value.checked_add(version.patch))
        .context("version is too large to encode as versionCode")?;

    let slot = match version.pre.is_empty() {
        true => FINAL_SLOT,
        false => candidate_slot(version)?,
    };

    base.checked_mul(VERSION_STRIDE)
        .and_then(|value| value.checked_add(slot))
        .context("version is too large to encode as versionCode")
}

/// Places a release candidate inside its version's window: one block of [`CANDIDATES_PER_STAGE`]
/// slots per stage, in the fixed order of [`CANDIDATE_STAGES`].
fn candidate_slot(version: &Version) -> Result<u64> {
    let (stage, number) = version
        .pre
        .as_str()
        .rsplit_once('.')
        .ok_or_else(|| unsupported_pre_release(version))?;

    // An unlisted stage has no fixed place among the listed ones, so it is refused rather than
    // guessed at. Inventing a stage name is the one mistake a maintainer is likely to make here.
    let stage_index = CANDIDATE_STAGES
        .iter()
        .position(|known| *known == stage)
        .ok_or_else(|| unsupported_pre_release(version))?;
    let stage = u64::try_from(stage_index).context("candidate stage index is too large")? + 1;

    // Semver has already rejected an empty identifier and a leading zero in a numeric one, so this
    // only has to place the number and keep `0` — which reads as "no candidate yet" — out.
    let number: u64 = number
        .parse()
        .with_context(|| unsupported_pre_release(version))?;
    if number == 0 || number >= CANDIDATES_PER_STAGE {
        bail!(unsupported_pre_release(version));
    }

    // The last candidate of the last stage must still land below its own finished release.
    stage
        .checked_mul(CANDIDATES_PER_STAGE)
        .and_then(|base| base.checked_add(number))
        .filter(|slot| *slot < FINAL_SLOT)
        .context("pre-release slot is too large")
}

/// Explains the one pre-release shape a `versionCode` can order.
fn unsupported_pre_release(version: &Version) -> anyhow::Error {
    let base = Version {
        pre: semver::Prerelease::EMPTY,
        ..version.clone()
    };
    anyhow::anyhow!(
        "pre-release '{version}' cannot be ordered as a versionCode; \
         tag it as '{base}-<stage>.<number>' with a stage from [{}] and a number from 1 to {}, \
         for example '{base}-rc.1'",
        CANDIDATE_STAGES.join(", "),
        CANDIDATES_PER_STAGE - 1
    )
}

/// Prints the tag's version facts for the workflow, one `key=value` per line.
fn print_release_version(tag: &str) -> Result<()> {
    let release = ReleaseVersion::parse(tag)?;
    println!("version={}", release.version);
    println!("version_code={}", release.version_code);
    println!("prerelease={}", release.prerelease);
    Ok(())
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

fn build_webui(flavor: Flavor) -> Result<()> {
    let root = workspace_root()?;
    let webui = root.join("webui");

    run_command("pnpm", &["install", "--frozen-lockfile"], &webui)?;
    let status = Command::new(if cfg!(windows) { "pnpm.cmd" } else { "pnpm" })
        .args(["run", "build"])
        .current_dir(&webui)
        .env("MODULE_ID", MODULE_ID)
        .env("WEBUI_FLAVOR", flavor.slug())
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
    /// The NDK's clang wrapper prefix, before the API level is appended.
    clang_prefix: &'static str,
    /// The API level the NDK ships this architecture's toolchain for.
    api_level: &'static str,
    /// Whether cargo-ndk knows this ABI. It rejects riscv64, which is built through
    /// the NDK directly instead.
    cargo_ndk: bool,
}

/// The supported architectures: cargo-ndk ABI, Rust target, in-zip filename suffix, NDK
/// clang prefix, NDK API level, and whether cargo-ndk builds it.
///
/// riscv64 is a Tier 3 Rust target: rustup ships no prebuilt `std` for it and cargo-ndk
/// does not list the ABI, so it is compiled straight through the NDK with `-Z build-std`.
/// The NDK only carries a riscv64 sysroot from r27 onward, and only for API 35.
const ANDROID_ARCHS: &[AndroidArch] = &[
    AndroidArch {
        ndk_abi: "arm64-v8a",
        rust_target: "aarch64-linux-android",
        suffix: "arm64",
        clang_prefix: "aarch64-linux-android",
        api_level: "26",
        cargo_ndk: true,
    },
    AndroidArch {
        ndk_abi: "armeabi-v7a",
        rust_target: "armv7-linux-androideabi",
        suffix: "armv7",
        clang_prefix: "armv7a-linux-androideabi",
        api_level: "26",
        cargo_ndk: true,
    },
    AndroidArch {
        ndk_abi: "x86_64",
        rust_target: "x86_64-linux-android",
        suffix: "x86_64",
        clang_prefix: "x86_64-linux-android",
        api_level: "26",
        cargo_ndk: true,
    },
    AndroidArch {
        ndk_abi: "riscv64",
        rust_target: "riscv64-linux-android",
        suffix: "riscv64",
        clang_prefix: "riscv64-linux-android",
        api_level: "35",
        cargo_ndk: false,
    },
];

/// The architectures cargo-ndk builds in one multitarget invocation.
fn cargo_ndk_archs(flavor: Flavor) -> impl Iterator<Item = &'static AndroidArch> {
    flavor.archs().filter(|arch| arch.cargo_ndk)
}

/// The architectures the NDK is driven for directly, because cargo-ndk has no ABI for them.
fn direct_ndk_archs(flavor: Flavor) -> impl Iterator<Item = &'static AndroidArch> {
    flavor.archs().filter(|arch| !arch.cargo_ndk)
}

fn rustup_target_args(flavor: Flavor) -> Vec<&'static str> {
    let mut args = vec!["target", "add"];
    // riscv64 is Tier 3 with no downloadable `std`, so it is left out here; its
    // standard library comes from `-Z build-std` against the rust-src component.
    args.extend(cargo_ndk_archs(flavor).map(|arch| arch.rust_target));
    args.extend(["--toolchain", "nightly"]);
    args
}

fn cargo_ndk_args(release: bool, flavor: Flavor) -> Vec<&'static str> {
    let mut args = vec!["+nightly", "ndk"];
    for arch in cargo_ndk_archs(flavor) {
        args.extend(["-t", arch.ndk_abi]);
    }
    args.extend(["--platform", "26", "build", "--bin", "hybrid-mount"]);
    if release {
        args.push("--release");
    }
    args
}

/// Builds one architecture through the NDK toolchain without cargo-ndk.
///
/// riscv64 has no prebuilt `std`, so `-Z build-std` compiles the standard library from
/// the nightly `rust-src` component. The NDK ships its riscv64 clang wrapper from r27
/// onward, for API 35 only.
fn compile_through_ndk(root: &Path, release: bool, arch: &AndroidArch) -> Result<()> {
    let host_tag = match std::env::consts::OS {
        "linux" => "linux-x86_64",
        "macos" => "darwin-x86_64",
        "windows" => "windows-x86_64",
        other => bail!("no Android NDK host toolchain for {other}"),
    };
    let ndk_home =
        env::var("ANDROID_NDK_HOME").context("ANDROID_NDK_HOME must be set to build riscv64")?;
    let toolchain_bin = Path::new(&ndk_home)
        .join("toolchains")
        .join("llvm")
        .join("prebuilt")
        .join(host_tag)
        .join("bin");
    let clang = toolchain_bin.join(format!("{}{}-clang", arch.clang_prefix, arch.api_level));
    if !clang.exists() {
        bail!(
            "{} not found; the Android NDK carries the riscv64 toolchain from r27 onward",
            clang.display()
        );
    }

    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let mut command = Command::new(&cargo);
    command.current_dir(root);
    command.args(["build", "--target", arch.rust_target]);
    command.args(["-Z", "build-std=std,panic_abort"]);
    if release {
        command.arg("--release");
    }
    let path = env::var("PATH").unwrap_or_default();
    command.env("PATH", format!("{}:{path}", toolchain_bin.display()));
    let env_target = arch.rust_target.replace('-', "_");
    command.env("CC", &clang);
    command.env(format!("CC_{env_target}"), &clang);
    command.env("AR", toolchain_bin.join("llvm-ar"));
    command.env(
        format!("CARGO_TARGET_{}_LINKER", env_target.to_uppercase()),
        &clang,
    );
    let status = command
        .status()
        .with_context(|| format!("failed to run cargo for {}", arch.rust_target))?;
    if !status.success() {
        bail!("compilation failed for {}", arch.rust_target);
    }
    Ok(())
}

fn compile_binaries(release: bool, flavor: Flavor) -> Result<Vec<(String, PathBuf)>> {
    let root = workspace_root()?;
    let profile = if release { "release" } else { "debug" };
    run_command("rustup", &rustup_target_args(flavor), &root)?;
    run_command("cargo", &cargo_ndk_args(release, flavor), &root)?;
    for arch in direct_ndk_archs(flavor) {
        compile_through_ndk(&root, release, arch)?;
    }

    let mut binaries = Vec::new();
    for arch in flavor.archs() {
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

fn render_module_prop(version: &str, version_code: u64, flavor: Flavor) -> String {
    let name = if flavor == Flavor::Wear {
        "Hybrid Mount Wear"
    } else {
        MODULE_NAME
    };
    let update_url = UPDATE_JSON_URL.replace("update.json", flavor.update_file());
    format!(
        "id={MODULE_ID}\n\
         name={name}\n\
         version={version}\n\
         versionCode={version_code}\n\
         author={MODULE_AUTHOR}\n\
         description={MODULE_DESCRIPTION}\n\
         updateJson={update_url}\n\
         packageFlavor={}\n\
         metamodule=1\n",
        flavor.slug()
    )
}

fn stage_module(root: &Path, flavor: Flavor, binaries: &[(String, PathBuf)]) -> Result<PathBuf> {
    let stage = root.join("output").join(format!("stage-{}", flavor.slug()));
    remove_dir_if_exists(&stage)?;
    fs::create_dir_all(stage.join("binaries"))?;

    fs_extra::dir::copy(
        root.join("module"),
        &stage,
        &CopyOptions::new().content_only(true),
    )
    .context("failed to stage module files")?;

    // Generated files are always selected explicitly; never inherit stale files
    // from a previous local build of another flavor or architecture.
    remove_dir_if_exists(&stage.join("webroot"))?;
    fs::create_dir_all(stage.join("webroot"))?;
    fs_extra::dir::copy(
        root.join("output").join(format!("webui-{}", flavor.slug())),
        stage.join("webroot"),
        &CopyOptions::new().content_only(true),
    )?;
    remove_dir_if_exists(&stage.join("binaries"))?;
    fs::create_dir_all(stage.join("binaries"))?;

    // The kernel sources ship with the package so installs stay GPL-complete, and so do
    // the prebuilt modules. A working tree that was built in place must not carry its
    // Kbuild output into the release, so that is stripped from both source trees.
    prune_kernel_build_output(&stage.join("vfs").join("src"))?;
    prune_kernel_build_output(&stage.join("lkm").join("src"))?;

    for (suffix, binary) in binaries {
        let staged_binary = stage
            .join("binaries")
            .join(format!("hybrid-mount-{suffix}"));
        fs::copy(binary, &staged_binary)?;
    }

    Ok(stage)
}

fn build(release: bool, flavor: Flavor) -> Result<()> {
    let root = workspace_root()?;
    vfs_package::verify(&root)?;
    let version = package_version()?;
    // The manifest version is already semver, so the shared encoder is the only part that has to
    // agree with the one the workflow uses for a tag.
    let parsed = Version::parse(&version)
        .with_context(|| format!("package.version '{version}' in Cargo.toml is not semver"))?;
    let version_code = version_code(&parsed)?;
    let commit_count = git_commit_count()?;

    build_webui(flavor)?;
    let binaries = compile_binaries(release, flavor)?;

    let stage = stage_module(&root, flavor, &binaries)?;

    let prop = render_module_prop(&version, version_code, flavor);
    fs::write(stage.join("module.prop"), prop)?;

    let output_dir = root.join("output");
    fs::create_dir_all(&output_dir)?;
    let suffix = if flavor == Flavor::Wear { "_Wear" } else { "" };
    let zip_name = format!("Hybrid-Mount{suffix}-{version}-{commit_count}.zip");
    let zip_path = output_dir.join(&zip_name);
    remove_file_if_exists(&zip_path)?;

    zip_create_from_directory_with_options(&zip_path, &stage, |_| FileOptions::default())?;

    println!("created {}", zip_path.display());
    Ok(())
}

fn notify(output: &Path, label: &str, topic_id: Option<i64>) -> Result<()> {
    if !["TELEGRAM_BOT_TOKEN", "TELEGRAM_CHAT_ID"]
        .iter()
        .all(|name| env::var(name).is_ok_and(|value| !value.trim().is_empty()))
    {
        println!("Telegram secrets not configured, skipping notification.");
        return Ok(());
    }
    let binary = env::var_os("HYBRID_MOUNT_NOTIFY").unwrap_or_else(|| "hybrid-mount-notify".into());
    let mut command = Command::new(binary);
    command
        .arg("--output")
        .arg(output)
        .arg("--label")
        .arg(label);
    if let Some(topic_id) = topic_id {
        command.arg("--topic-id").arg(topic_id.to_string());
    }
    let status = command
        .status()
        .context("install the prebuilt hybrid-mount-notify or set HYBRID_MOUNT_NOTIFY")?;
    if !status.success() {
        bail!("notification helper failed: {status}");
    }
    Ok(())
}

fn write_update_json(
    version: &str,
    version_code: u64,
    zip_url: &str,
    changelog: &str,
    flavor: Flavor,
) -> Result<()> {
    let root = workspace_root()?;
    let payload = serde_json::json!({
        "version": version,
        "versionCode": version_code,
        "zipUrl": zip_url,
        "changelog": changelog,
    });
    fs::write(
        root.join(flavor.update_file()),
        format!("{}\n", serde_json::to_string_pretty(&payload)?),
    )?;
    println!("{} written for {version}", flavor.update_file());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wear_build_has_arm_and_x86_64_targets_and_its_own_update_channel() {
        let args = cargo_ndk_args(true, Flavor::Wear);
        assert!(args.contains(&"arm64-v8a"));
        assert!(args.contains(&"armeabi-v7a"));
        assert!(args.contains(&"x86_64"));
        assert!(!args.contains(&"riscv64"));
        assert_eq!(Flavor::Wear.archs().count(), 3);
        let prop = render_module_prop("6.2.6", 602006999, Flavor::Wear);
        assert!(prop.contains("id=hybrid_mount\n"));
        assert!(prop.contains("name=Hybrid Mount Wear\n"));
        assert!(prop.contains("/update-wear.json\n"));
        assert!(prop.contains("packageFlavor=wear\n"));
        assert!(
            !render_module_prop("6.2.6", 602006999, Flavor::Normal).contains("update-wear.json")
        );
    }

    #[test]
    fn staging_isolates_frontends_and_removes_stale_binaries() {
        let root = tempfile::tempdir().expect("root");
        fs::create_dir_all(root.path().join("module/binaries")).expect("module");
        fs::create_dir_all(root.path().join("module/webroot")).expect("old frontend");
        fs::write(
            root.path().join("module/binaries/hybrid-mount-x86_64"),
            b"stale",
        )
        .expect("old binary");
        fs::write(root.path().join("module/webroot/phone.js"), b"stale").expect("old frontend");
        for flavor in [Flavor::Normal, Flavor::Wear] {
            let frontend = root.path().join(format!("output/webui-{}", flavor.slug()));
            fs::create_dir_all(&frontend).expect("frontend");
            fs::write(frontend.join("index.html"), flavor.slug()).expect("entry");
        }
        let binary = root.path().join("arm");
        fs::write(&binary, b"arm fixture").expect("binary");
        let x86_binary = root.path().join("x86");
        fs::write(&x86_binary, b"x86 fixture").expect("x86 binary");
        let stage = stage_module(
            root.path(),
            Flavor::Wear,
            &[("arm64".into(), binary), ("x86_64".into(), x86_binary)],
        )
        .expect("stage");
        assert_eq!(
            fs::read_to_string(stage.join("webroot/index.html")).expect("entry"),
            "wear"
        );
        assert!(!stage.join("webroot/phone.js").exists());
        assert_eq!(
            fs::read(stage.join("binaries/hybrid-mount-x86_64")).expect("x86 binary"),
            b"x86 fixture"
        );
        assert!(stage.join("binaries/hybrid-mount-arm64").is_file());
    }

    #[test]
    fn a_tag_with_a_leading_zero_patch_is_rejected() {
        let error =
            ReleaseVersion::parse("v6.2.01").expect_err("leading zero patch must be rejected");
        let message = format!("{error:#}");
        assert!(
            message.contains("v6.2.01"),
            "the error must name the offending tag, got: {message}"
        );
    }

    #[test]
    fn release_tags_are_validated_as_semver() {
        assert!(ReleaseVersion::parse("v6.2.1").is_ok());
        assert!(ReleaseVersion::parse("6.2.1").is_ok());
        assert!(ReleaseVersion::parse("v6.2").is_err());
        assert!(ReleaseVersion::parse("nightly").is_err());
        assert!(ReleaseVersion::parse("v6.2.1+build.5").is_err());
    }

    #[test]
    fn a_release_outranks_its_own_pre_releases() {
        let finished = ReleaseVersion::parse("v6.2.1").expect("release tag");
        assert!(!finished.prerelease);
        assert_eq!(finished.version, "6.2.1");

        for candidate in ["v6.2.1-alpha.1", "v6.2.1-beta.2", "v6.2.1-rc.99"] {
            let parsed = ReleaseVersion::parse(candidate).expect("candidate tag");
            assert!(parsed.prerelease, "{candidate} is a pre-release");
            assert!(
                parsed.version_code < finished.version_code,
                "{candidate} must not outrank the release it leads to"
            );
            assert!(
                parsed.version_code > finished.version_code - VERSION_STRIDE,
                "{candidate} must stay inside its own version's window"
            );
        }
    }

    #[test]
    fn pre_release_slots_order_by_stage_then_number() {
        let code = |tag: &str| ReleaseVersion::parse(tag).expect("tag").version_code;

        for (earlier, later) in [
            ("v6.2.1-alpha.1", "v6.2.1-alpha.2"),
            ("v6.2.1-alpha.9", "v6.2.1-beta.1"),
            ("v6.2.1-beta.9", "v6.2.1-rc.1"),
            ("v6.2.1-rc.2", "v6.2.1-rc.10"),
        ] {
            let left = code(earlier);
            let right = code(later);
            assert!(
                left < right,
                "{earlier} must precede {later}: {left} vs {right}"
            );
        }
    }

    #[test]
    fn unsupported_pre_release_shapes_are_refused() {
        for tag in [
            "v6.2.1-rc",
            "v6.2.1-rc.0",
            "v6.2.1-rc.1.2",
            "v6.2.1-preview.1",
            "v6.2.1-RC.1",
            "v6.2.1-rc.999",
        ] {
            let error = ReleaseVersion::parse(tag)
                .expect_err(&format!("{tag} must be refused, not approximated"));
            let message = format!("{error:#}");
            assert!(
                message.contains("-rc.1") && message.contains("stage"),
                "the error must show the supported shape, got: {message}"
            );
        }
    }

    #[test]
    fn invalid_semver_is_rejected_before_the_code_shape() {
        for tag in ["v6.2.1-rc.01", "v6.2.01", "v6.2.1-", "v6.2.1-rc..1"] {
            let error =
                ReleaseVersion::parse(tag).expect_err(&format!("{tag} must be refused as semver"));
            let message = format!("{error:#}");
            assert!(
                message.contains("not a semver version"),
                "the error must report the tag as invalid semver, got: {message}"
            );
        }
    }

    #[test]
    fn a_candidate_code_is_independent_of_other_tags() {
        let first = ReleaseVersion::parse("v6.2.1-rc.1").expect("tag");
        let second = ReleaseVersion::parse("v6.2.1-rc.2").expect("tag");

        assert_eq!(first.version_code + 1, second.version_code);
        assert_eq!(first.version, "6.2.1-rc.1");
        assert!(first.prerelease && second.prerelease);
    }

    #[test]
    fn version_codes_increase_across_versions() {
        let ordered = [
            "v6.1.4",
            "v6.2.0",
            "v6.2.1-alpha.1",
            "v6.2.1-rc.1",
            "v6.2.1",
            "v6.2.2-rc.1",
            "v6.3.0",
            "v7.0.0",
        ];
        let codes: Vec<u64> = ordered
            .iter()
            .map(|tag| ReleaseVersion::parse(tag).expect("tag").version_code)
            .collect();

        for pair in codes.windows(2) {
            assert!(pair[0] < pair[1], "{codes:?} must strictly increase");
        }
    }

    #[test]
    fn new_codes_outrank_the_published_scheme() {
        for (version, published) in [("6.2.0", 602_000), ("6.1.4", 601_004), ("6.0.0", 600_000)] {
            let code = ReleaseVersion::parse(version).expect("tag").version_code;
            assert!(
                code > published,
                "{version} encodes as {code}, which a device on {published} would not accept"
            );
        }
    }

    #[test]
    fn the_injected_version_stamps_the_same_code_as_the_tag() {
        for tag in ["v6.2.1", "v6.2.1-rc.1", "v6.2.1-beta.3", "v7.0.0"] {
            let release = ReleaseVersion::parse(tag).expect("tag");
            let injected = Version::parse(&release.version).expect("injected version is semver");
            assert_eq!(
                version_code(&injected).expect("code"),
                release.version_code,
                "{tag} must stamp module.prop and update.json with the same code"
            );
        }
    }

    #[test]
    fn android_build_uses_one_multitarget_cargo_ndk_invocation() {
        assert_eq!(
            rustup_target_args(Flavor::Normal),
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
            cargo_ndk_args(true, Flavor::Normal),
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
    fn riscv64_is_built_through_the_ndk_not_cargo_ndk() {
        let riscv = ANDROID_ARCHS
            .iter()
            .find(|arch| arch.rust_target == "riscv64-linux-android")
            .expect("riscv64 must be a supported architecture");
        assert!(!riscv.cargo_ndk);
        assert_eq!(riscv.suffix, "riscv64");
        assert_eq!(riscv.api_level, "35");
        assert_eq!(riscv.clang_prefix, "riscv64-linux-android");
        assert!(!rustup_target_args(Flavor::Normal).contains(&"riscv64-linux-android"));
        assert!(!cargo_ndk_args(false, Flavor::Normal).contains(&"riscv64"));
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

    #[test]
    fn ext4_kernel_modules_are_listed_in_their_manifest() {
        let root = workspace_root().expect("workspace root");
        let binaries = root.join("module/lkm/binaries");
        let listed = fs::read_to_string(binaries.join("list.txt")).expect("read ext4 manifest");
        for name in ["nuke-android12-5.10.ko", "nuke-android-4.14.ko"] {
            assert!(
                listed.contains(name),
                "{name} is missing from the ext4 manifest"
            );
        }
    }
}
