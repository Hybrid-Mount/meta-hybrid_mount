// meta-hybrid_mount/src/main.rs
mod config;
mod defs;
mod utils;

#[path = "magic_mount/mod.rs"]
mod magic_mount;
mod overlay_mount;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::fs;
use std::io::{BufRead, BufReader};
use std::process::Command;
use anyhow::{Result, Context};
use clap::{Parser, Subcommand};
use config::{Config, CONFIG_FILE_DEFAULT};
use rustix::mount::{unmount, UnmountFlags};
use serde::Serialize;

#[derive(Parser, Debug)]
#[command(name = "meta-hybrid", version, about = "Hybrid Mount Metamodule")]
struct Cli {
    #[arg(short = 'c', long = "config")]
    config: Option<PathBuf>,
    #[arg(short = 'm', long = "moduledir")]
    moduledir: Option<PathBuf>,
    #[arg(short = 't', long = "tempdir")]
    tempdir: Option<PathBuf>,
    #[arg(short = 's', long = "mountsource")]
    mountsource: Option<String>,
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,
    #[arg(short = 'p', long = "partitions", value_delimiter = ',')]
    partitions: Vec<String>,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    GenConfig {
        #[arg(short = 'o', long = "output", default_value = CONFIG_FILE_DEFAULT)]
        output: PathBuf,
    },
    ShowConfig,
    /// Output storage usage in JSON format
    Storage,
    /// List modules in JSON format
    Modules,
}

#[derive(Serialize)]
struct ModuleInfo {
    id: String,
    name: String,
    version: String,
    author: String,
    description: String,
    mode: String,
}

const BUILTIN_PARTITIONS: &[&str] = &["system", "vendor", "product", "system_ext", "odm", "oem"];

fn load_config(cli: &Cli) -> Result<Config> {
    if let Some(config_path) = &cli.config {
        return Config::from_file(config_path);
    }
    match Config::load_default() {
        Ok(config) => Ok(config),
        Err(e) => {
            if Path::new(CONFIG_FILE_DEFAULT).exists() {
                eprintln!("Error loading config: {:#}", e);
            }
            Ok(Config::default())
        }
    }
}

fn read_prop(path: &Path, key: &str) -> Option<String> {
    if let Ok(file) = fs::File::open(path) {
        let reader = BufReader::new(file);
        for line in reader.lines().flatten() {
            if line.starts_with(key) && line.chars().nth(key.len()) == Some('=') {
                return Some(line[key.len() + 1..].to_string());
            }
        }
    }
    None
}

// --- Dynamic Description Update (Catgirl Edition 🐱) ---
fn update_module_prop(storage_mode: &str, nuke_active: bool, overlay_count: usize, magic_count: usize) {
    let path = Path::new(defs::MODULE_PROP_FILE);
    if !path.exists() { 
        log::warn!("module.prop not found at {}, skipping description update", path.display());
        return; 
    }

    // Catgirl Style Formatter
    let mode_str = if storage_mode == "tmpfs" { "Tmpfs" } else { "Ext4" };
    let status_emoji = if storage_mode == "tmpfs" { "🐾" } else { "💿" };
    
    let nuke_str = if nuke_active { " | 肉垫: 开启 ✨" } else { "" };
    
    // Construct the cute string
    let new_desc = format!(
        "description=😋 运行中喵～ ({}) {} | Overlay: {} | Magic: {}{}", 
        mode_str, status_emoji, overlay_count, magic_count, nuke_str
    );

    // Read and replace
    let mut new_lines = Vec::new();
    match fs::read_to_string(path) {
        Ok(content) => {
            for line in content.lines() {
                if line.starts_with("description=") {
                    new_lines.push(new_desc.clone());
                } else {
                    new_lines.push(line.to_string());
                }
            }
            // Write back
            if let Err(e) = fs::write(path, new_lines.join("\n")) {
                log::error!("Failed to update module.prop: {}", e);
            } else {
                log::info!("Updated module.prop description (Meow!).");
            }
        },
        Err(e) => log::error!("Failed to read module.prop: {}", e),
    }
}

// --- Nuke Logic ---

fn get_android_version() -> Option<String> {
    let output = Command::new("getprop")
        .arg("ro.build.version.release")
        .output()
        .ok()?;
    String::from_utf8(output.stdout).ok().map(|s| s.trim().to_string())
}

fn try_load_nuke(mnt_point: &Path) -> bool {
    log::info!("Attempting to load Nuke LKM for stealth...");
    
    let uname = match utils::get_kernel_release() {
        Ok(v) => v,
        Err(e) => {
            log::error!("Failed to get kernel release: {}", e);
            return false;
        }
    };
    log::info!("Kernel release: {}", uname);

    let lkm_dir = Path::new(defs::MODULE_LKM_DIR);
    if !lkm_dir.exists() {
        log::warn!("LKM directory not found at {}", lkm_dir.display());
        return false;
    }

    let android_ver = get_android_version().unwrap_or_default();
    let parts: Vec<&str> = uname.split('.').collect();
    
    if parts.len() < 2 { return false; }
    let kernel_short = format!("{}.{}", parts[0], parts[1]); 

    let mut target_ko = None;
    let mut entries = Vec::new();
    
    if let Ok(dir) = fs::read_dir(lkm_dir) {
        for entry in dir.flatten() {
            entries.push(entry.path());
        }
    }

    if !android_ver.is_empty() {
        let pattern_android = format!("android{}", android_ver);
        for path in &entries {
            let name = path.file_name().unwrap().to_string_lossy();
            if name.contains(&kernel_short) && name.contains(&pattern_android) {
                target_ko = Some(path.clone());
                log::info!("Found exact match LKM: {}", name);
                break;
            }
        }
    }

    if target_ko.is_none() {
        for path in &entries {
            let name = path.file_name().unwrap().to_string_lossy();
            if name.contains(&kernel_short) {
                target_ko = Some(path.clone());
                log::info!("Found loose match LKM: {}", name);
                break;
            }
        }
    }

    let ko_path = match target_ko {
        Some(p) => p,
        None => {
            log::warn!("No matching Nuke LKM found for kernel {} (Android {})", uname, android_ver);
            return false;
        }
    };

    let _kptr_guard = utils::ScopedKptrRestrict::new();

    let cmd = Command::new("sh")
        .arg("-c")
        .arg("grep \" ext4_unregister_sysfs$\" /proc/kallsyms | awk '{print \"0x\"$1}'")
        .output();
        
    let sym_addr = match cmd {
        Ok(o) if o.status.success() => String::from_utf8(o.stdout).unwrap_or_default().trim().to_string(),
        _ => return false,
    };

    if sym_addr.is_empty() || sym_addr == "0x0000000000000000" {
        log::warn!("Symbol ext4_unregister_sysfs not found or masked.");
        return false;
    }

    log::info!("Symbol address: {}", sym_addr);

    let status = Command::new("insmod")
        .arg(ko_path)
        .arg(format!("mount_point={}", mnt_point.display()))
        .arg(format!("symaddr={}", sym_addr))
        .status();

    match status {
        Ok(_) => {
            // Success or EAGAIN (self-unload) is considered success for Nuke
            true
        },
        Err(e) => {
            log::error!("Failed to execute insmod: {}", e);
            false
        },
    }
}

// --- Smart Storage Logic ---

fn setup_storage(mnt_dir: &Path, image_path: &Path, force_ext4: bool) -> Result<String> {
    log::info!("Setting up storage at {}", mnt_dir.display());

    if force_ext4 {
        log::info!("Force Ext4 enabled. Skipping Tmpfs check.");
    } else {
        log::info!("Attempting Tmpfs mode...");
        if let Err(e) = utils::mount_tmpfs(mnt_dir) {
            log::warn!("Tmpfs mount failed: {}. Falling back to Image.", e);
        } else {
            if utils::is_xattr_supported(mnt_dir) {
                log::info!("Tmpfs mode active (XATTR supported).");
                return Ok("tmpfs".to_string());
            } else {
                log::warn!("Tmpfs does NOT support XATTR. Unmounting...");
                let _ = unmount(mnt_dir, UnmountFlags::DETACH);
            }
        }
    }

    log::info!("Falling back to Ext4 Image mode...");
    if !image_path.exists() {
        anyhow::bail!("modules.img not found at {}", image_path.display());
    }
    
    utils::mount_image(image_path, mnt_dir)
        .context("Failed to mount modules.img")?;
        
    log::info!("Image mode active.");
    Ok("ext4".to_string())
}

fn sync_active_modules(source_dir: &Path, target_base: &Path) -> Result<()> {
    log::info!("Syncing modules from {} to {}", source_dir.display(), target_base.display());
    let ids = scan_enabled_module_ids(source_dir)?;
    log::debug!("Found {} enabled modules to sync.", ids.len());
    
    for id in ids {
        let src = source_dir.join(&id);
        let dst = target_base.join(&id);
        let has_content = BUILTIN_PARTITIONS.iter().any(|p| src.join(p).exists());
        if has_content {
            log::debug!("Syncing module: {}", id);
            if let Err(e) = utils::sync_dir(&src, &dst) {
                log::error!("Failed to sync module {}: {}", id, e);
            }
        }
    }
    Ok(())
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB { format!("{:.1}G", bytes as f64 / GB as f64) }
    else if bytes >= MB { format!("{:.0}M", bytes as f64 / MB as f64) }
    else if bytes >= KB { format!("{:.0}K", bytes as f64 / KB as f64) }
    else { format!("{}B", bytes) }
}

fn check_storage() -> Result<()> {
    let mut path = PathBuf::from(defs::FALLBACK_CONTENT_DIR);
    if let Ok(state) = fs::read_to_string(defs::MOUNT_POINT_FILE) {
        let trimmed = state.trim();
        if !trimmed.is_empty() { path = PathBuf::from(trimmed); }
    }
    
    if !path.exists() {
        println!("{{ \"error\": \"Not mounted\" }}");
        return Ok(());
    }

    let stats = rustix::fs::statvfs(&path).context("statvfs failed")?;
    let block_size = stats.f_frsize as u64;
    let total_bytes = stats.f_blocks as u64 * block_size;
    let free_bytes = stats.f_bfree as u64 * block_size;
    let used_bytes = total_bytes.saturating_sub(free_bytes);
    let percent = if total_bytes > 0 { (used_bytes as f64 / total_bytes as f64) * 100.0 } else { 0.0 };
    println!("{{ \"size\": \"{}\", \"used\": \"{}\", \"percent\": \"{:.0}%\" }}", format_size(total_bytes), format_size(used_bytes), percent);
    Ok(())
}

fn list_modules(cli: &Cli) -> Result<()> {
    let config = load_config(cli)?;
    let module_modes = config::load_module_modes();
    let modules_dir = config.moduledir;
    let mut modules = Vec::new();

    let mut mnt_base = PathBuf::from(defs::FALLBACK_CONTENT_DIR);
    if let Ok(state) = fs::read_to_string(defs::MOUNT_POINT_FILE) {
        let trimmed = state.trim();
        if !trimmed.is_empty() { mnt_base = PathBuf::from(trimmed); }
    }

    if modules_dir.exists() {
        for entry in fs::read_dir(&modules_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() { continue; }
            let id = entry.file_name().to_string_lossy().to_string();
            if id == "meta-hybrid" || id == "lost+found" { continue; }
            if path.join(defs::DISABLE_FILE_NAME).exists() || path.join(defs::REMOVE_FILE_NAME).exists() || path.join(defs::SKIP_MOUNT_FILE_NAME).exists() { continue; }

            let has_content = BUILTIN_PARTITIONS.iter().any(|p| {
                path.join(p).exists() || mnt_base.join(&id).join(p).exists()
            });

            if has_content {
                let prop_path = path.join("module.prop");
                let name = read_prop(&prop_path, "name").unwrap_or_else(|| id.clone());
                let version = read_prop(&prop_path, "version").unwrap_or_default();
                let author = read_prop(&prop_path, "author").unwrap_or_default();
                let description = read_prop(&prop_path, "description").unwrap_or_default();
                let mode = module_modes.get(&id).cloned().unwrap_or_else(|| "auto".to_string());
                modules.push(ModuleInfo { id, name, version, author, description, mode });
            }
        }
    }
    modules.sort_by(|a, b| a.name.cmp(&b.name));
    println!("{}", serde_json::to_string(&modules)?);
    Ok(())
}

// --- Main Logic (Wrapped) ---

fn run() -> Result<()> {
    let cli = Cli::parse();

    if let Some(command) = &cli.command {
        match command {
            Commands::GenConfig { output } => { Config::default().save_to_file(output)?; return Ok(()); },
            Commands::ShowConfig => { 
                let config = load_config(&cli)?;
                println!("{}", serde_json::to_string(&config)?); 
                return Ok(()); 
            },
            Commands::Storage => { check_storage()?; return Ok(()); },
            Commands::Modules => { list_modules(&cli)?; return Ok(()); }
        }
    }

    let mut config = load_config(&cli)?;
    config.merge_with_cli(cli.moduledir, cli.tempdir, cli.mountsource, cli.verbose, cli.partitions);

    utils::init_logger(config.verbose, Path::new(defs::DAEMON_LOG_FILE))?;
    log::info!("Hybrid Mount Starting (True Hybrid Mode)...");

    utils::ensure_dir_exists(defs::RUN_DIR)?;

    // 1. Stealth Mount Point
    let mnt_base = if let Some(decoy) = utils::find_decoy_mount_point() {
        log::info!("Stealth Mode: Using decoy mount point at {}", decoy.display());
        decoy
    } else {
        log::warn!("Stealth Mode: No decoy found, falling back to default.");
        PathBuf::from(defs::FALLBACK_CONTENT_DIR)
    };

    if let Err(e) = fs::write(defs::MOUNT_POINT_FILE, mnt_base.to_string_lossy().as_bytes()) {
        log::error!("Failed to write mount state: {}", e);
    }

    let img_path = Path::new(defs::BASE_DIR).join("modules.img");
    if mnt_base.exists() { let _ = unmount(&mnt_base, UnmountFlags::DETACH); }

    // 2. Smart Storage Setup
    let storage_mode = setup_storage(&mnt_base, &img_path, config.force_ext4)?;
    
    // 3. Populate Storage
    if let Err(e) = sync_active_modules(&config.moduledir, &mnt_base) {
        log::error!("Critical: Failed to sync modules: {:#}", e);
    }

    // 4. Scan & Group
    let module_modes = config::load_module_modes();
    let mut active_modules: HashMap<String, PathBuf> = HashMap::new();
    if let Ok(entries) = fs::read_dir(&mnt_base) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                active_modules.insert(entry.file_name().to_string_lossy().to_string(), entry.path());
            }
        }
    }
    log::info!("Loaded {} modules from storage ({})", active_modules.len(), storage_mode);

    // 5. Partition Grouping & True Hybrid Logic
    let mut partition_overlay_map: HashMap<String, Vec<PathBuf>> = HashMap::new();
    let mut magic_mount_modules: HashSet<PathBuf> = HashSet::new();
    
    let mut all_partitions = BUILTIN_PARTITIONS.to_vec();
    let extra_parts: Vec<&str> = config.partitions.iter().map(|s| s.as_str()).collect();
    all_partitions.extend(extra_parts);

    // [FIX] Iterate by reference using &active_modules
    for (module_id, content_path) in &active_modules {
        let mode = module_modes.get(module_id).map(|s| s.as_str()).unwrap_or("auto");
        if mode == "magic" {
            magic_mount_modules.insert(content_path.clone());
            log::info!("Module '{}' assigned to Magic Mount", module_id);
        } else {
            for &part in &all_partitions {
                if content_path.join(part).is_dir() {
                    partition_overlay_map.entry(part.to_string()).or_default().push(content_path.clone());
                }
            }
        }
    }

    // Phase A: OverlayFS
    for (part, modules) in &partition_overlay_map {
        let target_path = format!("/{}", part);
        let overlay_paths: Vec<String> = modules.iter().map(|m| m.join(part).display().to_string()).collect();
        log::info!("Mounting {} [OVERLAY] ({} layers)", target_path, overlay_paths.len());
        if let Err(e) = overlay_mount::mount_overlay(&target_path, &overlay_paths, None, None) {
            log::error!("OverlayFS mount failed for {}: {:#}. Fallback to Magic.", target_path, e);
            for m in modules { magic_mount_modules.insert(m.clone()); }
        }
    }

    // [FIX] Capture count before move
    let magic_count = magic_mount_modules.len();

    // Phase B: Magic Mount
    if !magic_mount_modules.is_empty() {
        let tempdir = if let Some(t) = &config.tempdir { t.clone() } else { utils::select_temp_dir()? };
        log::info!("Starting Magic Mount Engine for {} modules...", magic_mount_modules.len());
        utils::ensure_temp_dir(&tempdir)?;
        let module_list: Vec<PathBuf> = magic_mount_modules.into_iter().collect();
        if let Err(e) = magic_mount::mount_partitions(&tempdir, &module_list, &config.mountsource, &config.partitions) {
            log::error!("Magic Mount failed: {:#}", e);
        }
        utils::cleanup_temp_dir(&tempdir);
    }

    // Phase C: Nuke LKM
    let mut nuke_active = false;
    if storage_mode == "ext4" && config.enable_nuke {
        nuke_active = try_load_nuke(&mnt_base);
    }

    // Update module description with stats (Catgirl Mode)
    // active_modules is still valid because we iterated by reference earlier
    let overlay_count = active_modules.len().saturating_sub(magic_count);
    
    update_module_prop(&storage_mode, nuke_active, overlay_count, magic_count);

    log::info!("Hybrid Mount Completed");
    Ok(())
}

fn scan_enabled_module_ids(metadata_dir: &Path) -> Result<Vec<String>> {
    let mut ids = Vec::new();
    if !metadata_dir.exists() { return Ok(ids); }
    for entry in fs::read_dir(metadata_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() { continue; }
        let id = entry.file_name().to_string_lossy().to_string();
        if id == "meta-hybrid" || id == "lost+found" { continue; }
        if path.join(defs::DISABLE_FILE_NAME).exists() || path.join(defs::REMOVE_FILE_NAME).exists() || path.join(defs::SKIP_MOUNT_FILE_NAME).exists() { continue; }
        ids.push(id);
    }
    Ok(ids)
}

fn main() {
    if let Err(e) = run() {
        log::error!("Fatal Error: {:#}", e);
        eprintln!("Fatal Error: {:#}", e);
        std::process::exit(1);
    }
}
