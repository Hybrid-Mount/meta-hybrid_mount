mod zip_ext;

use std::{
    env,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::Result;
use clap::{Parser, Subcommand};
use fs_extra::dir;
use zip::{write::FileOptions, CompressionMethod};

use crate::zip_ext::zip_create_from_directory_with_options;

#[derive(Parser)]
#[command(name = "xtask")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build the full project
    Build {
        /// Build in release mode
        #[arg(long)]
        release: bool,
        /// Path to signing private key (PEM format)
        #[arg(long)]
        sign_key: Option<PathBuf>,
        /// Path to external, pre-compiled zakosign binary
        #[arg(long)]
        zakosign_bin: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = project_root();

    match cli.command {
        Commands::Build { release, sign_key, zakosign_bin } => {
            let output_dir = root.join("output");
            let module_build_dir = output_dir.join("module_files");

            // 1. Clean & Setup
            println!(":: Cleaning output directory...");
            if output_dir.exists() {
                fs::remove_dir_all(&output_dir)?;
            }
            fs::create_dir_all(&module_build_dir)?;

            // 2. Build WebUI
            build_webui(&root)?;

            // 3. Build Core (Android)
            let core_bin = build_core(&root, release)?;

            // 4. Copy Module Files
            println!(":: Copying module files...");
            let module_src = root.join("module");
            dir::copy(
                &module_src,
                &module_build_dir, 
                &dir::CopyOptions::new().overwrite(true).content_only(true),
            )?;
            
            // Cleanup gitignore
            let gitignore = module_build_dir.join(".gitignore");
            if gitignore.exists() { fs::remove_file(gitignore)?; }

            // 5. Inject Version
            let version = inject_version(&module_build_dir)?;
            fs::write(output_dir.join("version"), &version)?;

            // 6. Install Core Binary
            let dest_bin = module_build_dir.join("meta-hybrid");
            fs::copy(&core_bin, &dest_bin)?;

            // 7. Zip Package
            println!(":: Creating zip archive...");
            let options = FileOptions::default()
                .compression_method(CompressionMethod::Deflated)
                .compression_level(Some(9));
            
            let zip_name = format!("meta-hybrid-{}.zip", version);
            let zip_path = output_dir.join(&zip_name);
            
            // Zip creates from module_build_dir
            zip_create_from_directory_with_options(
                &zip_path,
                &module_build_dir,
                |_| options,
            )?;

            // 8. Signing Logic (Using external zakosign binary)
            if let Some(bin_path) = zakosign_bin {
                if let Some(key_path) = sign_key {
                    println!(":: Signing module zip using external zakosign...");
                    
                    if !bin_path.exists() {
                        anyhow::bail!("Zakosign binary not found at: {}", bin_path.display());
                    }

                    // FIX: Updated command arguments to match zakosign usage:
                    // Usage: zakosign sign [options...] --key <private.key> <input>
                    let status = Command::new(&bin_path)
                        .arg("sign")
                        .arg("--key")      // Explicit flag for key
                        .arg(&key_path)    // The key path
                        .arg(&zip_path)    // The input file (zip)
                        .status()?;

                    if !status.success() {
                        anyhow::bail!("Failed to sign module zip. Exit code: {:?}", status.code());
                    }
                    println!(":: Signature applied successfully.");
                } else {
                    println!(":: [WARNING] zakosign binary provided but 'sign_key' is missing. Skipping signature.");
                }
            } else if sign_key.is_some() {
                 println!(":: [WARNING] sign_key provided but 'zakosign_bin' is missing. Skipping signature.");
            }

            println!(":: Build success: {}", zip_path.display());
        }
    }

    Ok(())
}

fn project_root() -> PathBuf {
    Path::new(&env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(1)
        .unwrap()
        .to_path_buf()
}

fn build_webui(root: &Path) -> Result<()> {
    println!(":: Building WebUI...");
    let webui_dir = root.join("webui");
    let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };

    let status = Command::new(npm)
        .current_dir(&webui_dir)
        .arg("install")
        .status()?;
    if !status.success() { anyhow::bail!("npm install failed"); }

    let status = Command::new(npm)
        .current_dir(&webui_dir)
        .args(["run", "build"])
        .status()?;
    if !status.success() { anyhow::bail!("npm run build failed"); }

    Ok(())
}

fn build_core(root: &Path, release: bool) -> Result<PathBuf> {
    println!(":: Building Meta-Hybrid Core (aarch64-linux-android)...");
    
    let mut cmd = Command::new("cargo");
    cmd.current_dir(root)
       .args(["ndk", "--platform", "30", "-t", "arm64-v8a", "build"]);
    
    if release {
        cmd.arg("--release");
    }
    
    cmd.env("RUSTFLAGS", "-C default-linker-libraries");
    
    let status = cmd.status()?;
    if !status.success() { anyhow::bail!("Cargo build failed"); }

    let profile = if release { "release" } else { "debug" };
    let bin_path = root.join("target/aarch64-linux-android")
        .join(profile)
        .join("meta-hybrid");
        
    if !bin_path.exists() {
        anyhow::bail!("Core binary not found at {}", bin_path.display());
    }

    Ok(bin_path)
}

fn inject_version(target_dir: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output();

    let hash = match output {
        Ok(o) if o.status.success() => String::from_utf8(o.stdout)?.trim().to_string(),
        _ => "unknown".to_string(),
    };

    let prop_path = target_dir.join("module.prop");
    let mut full_version = format!("v0.0.0-g{}", hash);

    if prop_path.exists() {
        let content = fs::read_to_string(&prop_path)?;
        let mut new_lines = Vec::new();
        
        for line in content.lines() {
            if line.starts_with("version=") {
                let base = line.trim().strip_prefix("version=").unwrap_or("");
                full_version = format!("{}-g{}", base, hash);
                new_lines.push(format!("version={}", full_version));
            } else {
                new_lines.push(line.to_string());
            }
        }
        
        fs::write(prop_path, new_lines.join("\n"))?;
        println!(":: Injected version: {}", full_version);
    }
    
    Ok(full_version)
}
