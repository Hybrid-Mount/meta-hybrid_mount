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
        Commands::ReleaseVersion { tag } => print_release_version(&tag),
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
    // The manifest version is already semver, so the shared encoder is the only part that has to
    // agree with the one the workflow uses for a tag.
    let parsed = Version::parse(&version)
        .with_context(|| format!("package.version '{version}' in Cargo.toml is not semver"))?;
    let version_code = version_code(&parsed)?;
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

    /// The release that failed: `v6.2.01` passed the workflow's own pattern, was written into
    /// `Cargo.toml`, and only then did Cargo reject it as `invalid leading zero in patch version
    /// number` — after the tag had already been pushed. It has to be refused up front, by name.
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

    /// Tags that are not versions at all, and tags with no `v`, are handled explicitly rather than
    /// falling through to a build step.
    #[test]
    fn release_tags_are_validated_as_semver() {
        assert!(ReleaseVersion::parse("v6.2.1").is_ok());
        assert!(ReleaseVersion::parse("6.2.1").is_ok());
        assert!(ReleaseVersion::parse("v6.2").is_err());
        assert!(ReleaseVersion::parse("nightly").is_err());
        assert!(ReleaseVersion::parse("v6.2.1+build.5").is_err());
    }

    /// A finished release must carry a higher code than its own candidates, or a device on a
    /// candidate would never be offered the release it led to.
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

    /// Candidates order against each other by stage and number, never lexically, so `-rc.10` is
    /// genuinely later than `-rc.2`.
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

    /// Every pre-release shape a `versionCode` cannot order exactly has to be refused with a message
    /// that says what to write instead, rather than being given an approximate code.
    #[test]
    fn unsupported_pre_release_shapes_are_refused() {
        for tag in [
            "v6.2.1-rc",        // no number
            "v6.2.1-rc.0",      // zero reads as "no candidate"
            "v6.2.1-rc.1.2",    // more than one identifier
            "v6.2.1-preview.1", // a stage the code cannot place
            "v6.2.1-RC.1",      // stages are matched exactly
            "v6.2.1-rc.999",    // past the numbers one stage reserves
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

    /// A shape that is not valid semver at all is refused by the semver parser before the
    /// `versionCode` shape is considered. Either way the tag never reaches a build step.
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

    /// A candidate's code must not depend on which other tags happen to exist, so tagging a later
    /// candidate never rewrites the code of an earlier one.
    #[test]
    fn a_candidate_code_is_independent_of_other_tags() {
        let first = ReleaseVersion::parse("v6.2.1-rc.1").expect("tag");
        let second = ReleaseVersion::parse("v6.2.1-rc.2").expect("tag");

        assert_eq!(first.version_code + 1, second.version_code);
        assert_eq!(first.version, "6.2.1-rc.1");
        assert!(first.prerelease && second.prerelease);
    }

    /// The code has to keep rising across every kind of release, including the one this repository
    /// just shipped: a candidate of the next version still has to outrank the finished prior one.
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

    /// A device already running a published release has to see the next one as an update, which
    /// only holds while every code this produces is above the whole-number code already published.
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

    /// The workflow injects `ReleaseVersion::version` into `Cargo.toml` and the build reads it back
    /// to stamp `module.prop`, while the workflow separately publishes the tag's code in
    /// `update.json`. A device compares those two, so if they ever disagreed the updater would
    /// either offer the same release forever or never offer it at all.
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
