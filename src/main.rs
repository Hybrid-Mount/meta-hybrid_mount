mod conf;
mod core;
mod defs;
mod mount;
#[cfg(any(target_os = "linux", target_os = "android"))]
mod try_umount;
mod utils;

use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use clap::Parser;
use mimalloc::MiMalloc;
use serde::Serialize;

use conf::{
    cli::{Cli, Commands},
    config::{Config, CONFIG_FILE_DEFAULT},
};
use core::{
    executor,
    inventory,
    planner,
    state::RuntimeState,
    storage,
    sync,
    modules,
};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[derive(Serialize)]
struct DiagnosticIssueJson {
    level: String,
    context: String,
    message: String,
}

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

fn check_zygisksu_enforce_status() -> bool {
    std::fs::read_to_string("/data/adb/zygisksu/denylist_enforce")
        .map(|s| s.trim() != "0")
        .unwrap_or(false)
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    if let Some(command) = &cli.command {
        match command {
            Commands::GenConfig { output } => { 
                Config::default().save_to_file(output)?; 
                return Ok(()); 
            },
            Commands::ShowConfig => { 
                let config = load_config(&cli)?;
                println!("{}", serde_json::to_string(&config)?); 
                return Ok(()); 
            },
            Commands::SaveConfig { payload } => {
                let json_bytes = (0..payload.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&payload[i..i + 2], 16))
                    .collect::<Result<Vec<u8>, _>>()
                    .context("Failed to decode hex payload")?;
                let config: Config = serde_json::from_slice(&json_bytes)
                    .context("Failed to parse config JSON")?;
                config.save_to_file(CONFIG_FILE_DEFAULT)?;
                println!("Configuration saved successfully.");
                return Ok(());
            },
            Commands::SaveRules { module, payload } => {
                let json_bytes = (0..payload.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&payload[i..i + 2], 16))
                    .collect::<Result<Vec<u8>, _>>()
                    .context("Failed to decode hex payload")?;
                let _: inventory::ModuleRules = serde_json::from_slice(&json_bytes)
                    .context("Invalid rules JSON")?;
                let rules_dir = std::path::Path::new("/data/adb/meta-hybrid/rules");
                std::fs::create_dir_all(rules_dir)?;
                let file_path = rules_dir.join(format!("{}.json", module));
                std::fs::write(file_path, json_bytes)?;
                println!("Rules for module '{}' saved.", module);
                return Ok(());
            },
            Commands::Storage => { 
                storage::print_status()?; 
                return Ok(()); 
            },
            Commands::Modules => { 
                let config = load_config(&cli)?;
                modules::print_list(&config)?; 
                return Ok(()); 
            },
            Commands::Conflicts => {
                let config = load_config(&cli)?;
                let module_list = inventory::scan(&config.moduledir, &config)?;
                let plan = planner::generate(&config, &module_list, &config.moduledir)?;
                let report = plan.analyze_conflicts();
                println!("{}", serde_json::to_string(&report.details)?);
                return Ok(());
            },
            Commands::Diagnostics => {
                let config = load_config(&cli)?;
                let module_list = inventory::scan(&config.moduledir, &config)?;
                let plan = planner::generate(&config, &module_list, &config.moduledir)?;
                let issues = executor::diagnose_plan(&plan);
                let json_issues: Vec<DiagnosticIssueJson> = issues.into_iter().map(|i| DiagnosticIssueJson {
                    level: match i.level {
                        executor::DiagnosticLevel::Info => "Info".to_string(),
                        executor::DiagnosticLevel::Warning => "Warning".to_string(),
                        executor::DiagnosticLevel::Critical => "Critical".to_string(),
                    },
                    context: i.context,
                    message: i.message,
                }).collect();
                println!("{}", serde_json::to_string(&json_issues)?);
                return Ok(());
            }
        }
    }

    let mut config = load_config(&cli)?;
    config.merge_with_cli(
        cli.moduledir.clone(), 
        cli.tempdir.clone(), 
        cli.mountsource.clone(), 
        cli.verbose, 
        cli.partitions.clone(),
        cli.dry_run,
    );

    if check_zygisksu_enforce_status() {
        if config.allow_umount_coexistence {
            if config.verbose {
                println!(">> ZygiskSU Enforce!=0 detected, but Umount Coexistence enabled. Respecting user config.");
            }
        } else {
            if config.verbose {
                println!(">> ZygiskSU Enforce!=0 detected. Forcing DISABLE_UMOUNT to TRUE.");
            }
            config.disable_umount = true;
        }
    }

    if config.dry_run {
        env_logger::builder()
            .filter_level(if config.verbose { log::LevelFilter::Debug } else { log::LevelFilter::Info })
            .init();
        
        log::info!(":: DRY-RUN / DIAGNOSTIC MODE ::");
        let module_list = inventory::scan(&config.moduledir, &config)?;
        log::info!(">> Inventory: Found {} modules", module_list.len());
        
        let plan = planner::generate(&config, &module_list, &config.moduledir)?;
        plan.print_visuals();
        
        log::info!(">> Analyzing File Conflicts...");
        let report = plan.analyze_conflicts();
        if report.details.is_empty() {
            log::info!("   No file conflicts detected. Clean.");
        } else {
            log::warn!("!! DETECTED {} FILE CONFLICTS !!", report.details.len());
            for c in report.details {
                log::warn!("   [{}] {} <== {:?}", c.partition, c.relative_path, c.contending_modules);
            }
        }

        log::info!(">> Running System Diagnostics...");
        let issues = executor::diagnose_plan(&plan);
        let mut critical_count = 0;
        for issue in issues {
            match issue.level {
                core::executor::DiagnosticLevel::Critical => {
                    log::error!("[CRITICAL][{}] {}", issue.context, issue.message);
                    critical_count += 1;
                },
                core::executor::DiagnosticLevel::Warning => {
                    log::warn!("[WARN][{}] {}", issue.context, issue.message);
                },
                core::executor::DiagnosticLevel::Info => {
                    log::info!("[INFO][{}] {}", issue.context, issue.message);
                }
            }
        }

        if critical_count > 0 {
            log::error!(">> ❌ DIAGNOSTICS FAILED: {} critical issues found.", critical_count);
            log::error!(">> Mounting now would likely result in a bootloop.");
            std::process::exit(1);
        } else {
            log::info!(">> ✅ Diagnostics passed. System looks healthy.");
        }
        return Ok(());
    }

    let _log_guard = utils::init_logging(config.verbose, Path::new(defs::DAEMON_LOG_FILE))?;
    
    #[cfg(any(target_os = "linux", target_os = "android"))]
    if crate::mount::hymofs::HymoFs::is_available() {
        if let Err(e) = crate::mount::hymofs::HymoFs::set_debug(config.verbose) {
            log::warn!("Failed to set HymoFS debug mode: {}", e);
        }
    }
    
    let camouflage_name = utils::random_kworker_name();
    if let Err(e) = utils::camouflage_process(&camouflage_name) {
        log::warn!("Failed to camouflage process: {}", e);
    }

    log::info!(">> Initializing Meta-Hybrid Mount Daemon...");
    log::debug!("Process camouflaged as: {}", camouflage_name);

    if config.disable_umount {
        log::warn!("!! Umount is DISABLED via config.");
    }

    utils::ensure_dir_exists(defs::RUN_DIR)?;

    let mnt_base = PathBuf::from(defs::FALLBACK_CONTENT_DIR);
    let img_path = Path::new(defs::BASE_DIR).join("modules.img");
    
    let storage_handle = storage::setup(&mnt_base, &img_path, config.force_ext4, &config.mountsource)?;
    log::info!(">> Storage Backend: [{}]", storage_handle.mode.to_uppercase());

    let module_list = inventory::scan(&config.moduledir, &config)?;
    log::info!(">> Inventory Scan: Found {} enabled modules.", module_list.len());
    
    sync::perform_sync(&module_list, &storage_handle.mount_point)?;

    let plan = planner::generate(&config, &module_list, &storage_handle.mount_point)?;
    plan.print_visuals();

    let active_mounts: Vec<String> = plan.overlay_ops
        .iter()
        .map(|op| op.partition_name.clone())
        .collect();

    log::info!(">> Link Start! Executing mount plan...");
    
    let exec_result = executor::execute(&plan, &config)?;

    let final_magic_ids = exec_result.magic_module_ids;
    
    let mut nuke_active = false;
    if storage_handle.mode == "ext4" && config.enable_nuke {
        log::info!(">> Engaging Paw Pad Protocol (Stealth)...");
        match utils::ksu_nuke_sysfs(storage_handle.mount_point.to_string_lossy().as_ref()) {
            Ok(_) => {
                log::info!(">> Success: Paw Pad active. Sysfs traces purged.");
                nuke_active = true;
            },
            Err(e) => {
                log::warn!("!! Paw Pad failure: {}", e);
            }
        }
    }

    modules::update_description(
        &storage_handle.mode, 
        nuke_active, 
        exec_result.overlay_module_ids.len(), 
        final_magic_ids.len(),
        exec_result.hymo_module_ids.len()
    );

    let storage_stats = storage::get_usage(&storage_handle.mount_point);
    let hymofs_available = storage::is_hymofs_active();
    
    #[cfg(any(target_os = "linux", target_os = "android"))]
    let hymofs_version = crate::mount::hymofs::HymoFs::get_version();
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    let hymofs_version = None;
    
    let state = RuntimeState::new(
        storage_handle.mode,
        storage_handle.mount_point,
        exec_result.overlay_module_ids,
        final_magic_ids,
        exec_result.hymo_module_ids,
        nuke_active,
        active_mounts,
        storage_stats,
        hymofs_available,
        hymofs_version
    );

    if let Err(e) = state.save() {
        log::error!("Failed to save runtime state: {}", e);
    }

    log::info!(">> System operational. Mount sequence complete.");
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }
}
