mod zip_ext;

use std::{
    env,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::{Context, Result};
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
    /// Build the full project with zakosign integration
    Build {
        /// Build in release mode
        #[arg(long)]
        release: bool,
        /// Path to signing private key (PEM). If not provided, tries ZAKOSIGN_KEY env var.
        #[arg(long)]
        sign_key: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = project_root();

    match cli.command {
        Commands::Build { release, sign_key } => {
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

            // 3. Build Zakosign (Host Tool)
            let zakosign_bin = build_zakosign(&root)?;

            // 4. Build Core (Android)
            let core_bin = build_core(&root, release)?;

            // 5. Copy Module Files
            println!(":: Copying module files...");
            let module_src = root.join("module");
            // Ensure module.prop, customize.sh, etc. are at the root of the zip
            // This also includes the freshly built webroot
            dir::copy(
                &module_src,
                &output_dir,
                &dir::CopyOptions::new().overwrite(true).content_only(true),
            )?;
            
            // Cleanup gitignore if copied
            let gitignore = module_build_dir.join(".gitignore");
            if gitignore.exists() { fs::remove_file(gitignore)?; }

            // 6. Inject Version
            let version = inject_version(&module_build_dir)?;
            fs::write(output_dir.join("version"), &version)?;

            // 7. Install & Sign Core Binary
            let dest_bin = module_build_dir.join("meta-hybrid");
            fs::copy(&core_bin, &dest_bin)?;

            if let Some(zakosign) = zakosign_bin {
                if let Some(key) = resolve_sign_key(sign_key) {
                    println!(":: Signing meta-hybrid binary...");
                    // Make binary executable just in case (Unix only)
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        if let Ok(metadata) = fs::metadata(&zakosign) {
                            let mut perms = metadata.permissions();
                            perms.set_mode(0o755);
                            let _ = fs::set_permissions(&zakosign, perms);
                        }
                    }

                    let status = Command::new(zakosign)
                        .current_dir(&root)
                        .arg("sign")
                        .arg(&dest_bin)
                        .arg("--key")
                        .arg(key)
                        .arg("--output")
                        .arg(&dest_bin)
                        .arg("--force")
                        .stdout(Stdio::inherit())
                        .stderr(Stdio::inherit())
                        .status()
                        .context("Failed to execute zakosign")?;

                    if !status.success() {
                        anyhow::bail!("Zakosign signing failed!");
                    }
                    println!(":: Binary signed successfully.");
                } else {
                    println!(":: [WARNING] No signing key found (ZAKOSIGN_KEY or --sign-key). Skipping signature.");
                }
            } else {
                println!(":: [WARNING] Zakosign binary not built. Skipping signature.");
            }

            // 8. Zip Package
            println!(":: Creating zip archive...");
            let options = FileOptions::default()
                .compression_method(CompressionMethod::Deflated)
                .compression_level(Some(9));
            
            let zip_name = format!("meta-hybrid-{}.zip", version);
            let zip_path = output_dir.join(zip_name);
            
            zip_create_from_directory_with_options(
                &zip_path,
                &module_build_dir,
                |_| options,
            )?;

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

fn build_zakosign(root: &Path) -> Result<Option<PathBuf>> {
    let zakosign_dir = root.join("zakosign");
    if !zakosign_dir.exists() {
        println!(":: [INFO] zakosign directory not found at {}. Skipping zakosign build.", zakosign_dir.display());
        return Ok(None);
    }

    println!(":: Building Zakosign (Host)...");
    
    let setup_script = zakosign_dir.join("tools/setupdep");
    if setup_script.exists() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = fs::metadata(&setup_script) {
                let mut perms = metadata.permissions();
                perms.set_mode(0o755);
                let _ = fs::set_permissions(&setup_script, perms);
            }
        }

        println!(":: [Zakosign] Compiling dependencies (Host Only)...");
        let status_build = Command::new(&setup_script)
            .current_dir(&zakosign_dir)
            .env_remove("ANDROID_NDK_HOME")
            .env_remove("ANDROID_NDK_ROOT")
            .env_remove("ANDROID_NDK")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?;
            
        if !status_build.success() { 
            anyhow::bail!("zakosign setupdep (build) failed"); 
        }
        
        println!(":: [Zakosign] Configuring host environment...");
        let status_host = Command::new(&setup_script)
            .current_dir(&zakosign_dir)
            .arg("host")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?;
        if !status_host.success() { 
            anyhow::bail!("zakosign setupdep (host) failed"); 
        }
    }

    println!(":: [Zakosign] Compiling binary...");
    let status = Command::new("make")
        .current_dir(&zakosign_dir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;
    
    if !status.success() { anyhow::bail!("zakosign make failed"); }

    let bin_path = zakosign_dir.join("bin/zakosign");
    if bin_path.exists() {
        Ok(Some(bin_path))
    } else {
        anyhow::bail!("zakosign binary not found after build at {}", bin_path.display());
    }
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

fn resolve_sign_key(arg_key: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(k) = arg_key {
        return Some(k);
    }
    if let Ok(k) = env::var("ZAKOSIGN_KEY") {
        return Some(PathBuf::from(k));
    }
    None
}
