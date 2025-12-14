use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use anyhow::Result;
use serde::Serialize;
use walkdir::WalkDir;
use crate::{conf::config, defs, core::inventory::{Module, MountMode}};

#[derive(Debug, Clone)]
pub struct OverlayOperation {
    pub partition_name: String,
    pub target: String,
    pub lowerdirs: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct HymoOperation {
    pub module_id: String,
    pub source: PathBuf,
    pub target: PathBuf,
}

#[derive(Debug, Default)]
pub struct MountPlan {
    pub overlay_ops: Vec<OverlayOperation>,
    pub hymo_ops: Vec<HymoOperation>,
    pub magic_module_paths: Vec<PathBuf>,
    pub overlay_module_ids: Vec<String>,
    pub hymo_module_ids: Vec<String>,
    pub magic_module_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConflictEntry {
    pub partition: String,
    pub relative_path: String,
    pub contending_modules: Vec<String>,
}

#[derive(Debug, Default)]
pub struct ConflictReport {
    pub details: Vec<ConflictEntry>,
}

impl MountPlan {
    pub fn analyze_conflicts(&self) -> ConflictReport {
        let mut conflicts = Vec::new();
        for op in &self.overlay_ops {
            let mut file_map: HashMap<String, Vec<String>> = HashMap::new();
            for layer_path in &op.lowerdirs {
                let module_id = layer_path.parent()
                    .and_then(|p| p.file_name())
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "UNKNOWN".into());
                
                for entry in WalkDir::new(layer_path).min_depth(1) {
                    if let Ok(entry) = entry {
                        if !entry.file_type().is_file() { continue; }
                        if let Ok(rel) = entry.path().strip_prefix(layer_path) {
                            let rel_str = rel.to_string_lossy().to_string();
                            file_map.entry(rel_str).or_default().push(module_id.clone());
                        }
                    }
                }
            }

            for (rel_path, modules) in file_map {
                if modules.len() > 1 {
                    conflicts.push(ConflictEntry {
                        partition: op.partition_name.clone(),
                        relative_path: rel_path,
                        contending_modules: modules,
                    });
                }
            }
        }

        conflicts.sort_by(|a, b| {
            a.partition.cmp(&b.partition)
                .then_with(|| a.relative_path.cmp(&b.relative_path))
        });

        ConflictReport { details: conflicts }
    }

    pub fn print_visuals(&self) {
        if self.overlay_ops.is_empty() && self.magic_module_paths.is_empty() && self.hymo_ops.is_empty() {
            log::info!(">> Empty plan. Standby mode.");
            return;
        }

        if !self.hymo_ops.is_empty() {
            log::info!("[HymoFS Injection Protocol]");
            let mut shown_modules = HashSet::new();
            for op in &self.hymo_ops {
                if shown_modules.insert(&op.module_id) {
                    log::info!("├── [Inject] {}", op.module_id);
                }
            }
        }

        if !self.overlay_ops.is_empty() {
            log::info!("[OverlayFS Fusion Sequence]");
            for (i, op) in self.overlay_ops.iter().enumerate() {
                let is_last_op = i == self.overlay_ops.len() - 1 && self.magic_module_paths.is_empty();
                let branch = if is_last_op { "╰──" } else { "├──" };
                
                log::info!("{} [Target: {}] {}", branch, op.partition_name, op.target);
                
                let prefix = if is_last_op { "    " } else { "│   " };
                for (j, layer) in op.lowerdirs.iter().enumerate() {
                    let is_last_layer = j == op.lowerdirs.len() - 1;
                    let sub_branch = if is_last_layer { "╰──" } else { "├──" };
                    let mod_name = layer.parent()
                        .and_then(|p| p.file_name())
                        .map(|n| n.to_string_lossy())
                        .unwrap_or_else(|| "UNKNOWN".into());
                        
                    log::info!("{}{} [Layer] {}", prefix, sub_branch, mod_name);
                }
            }
        }

        if !self.magic_module_paths.is_empty() {
            log::info!("[Magic Mount Fallback Protocol]");
            for (i, path) in self.magic_module_paths.iter().enumerate() {
                let is_last = i == self.magic_module_paths.len() - 1;
                let branch = if is_last { "╰──" } else { "├──" };
                let mod_name = path.file_name()
                    .map(|n| n.to_string_lossy())
                    .unwrap_or_else(|| "UNKNOWN".into());
                log::info!("{} [Bind] {}", branch, mod_name);
            }
        }
    }
}

pub fn generate(
    config: &config::Config, 
    modules: &[Module], 
    storage_root: &Path
) -> Result<MountPlan> {
    let mut plan = MountPlan::default();
    let mut overlay_groups: HashMap<String, Vec<PathBuf>> = HashMap::new();
    let mut magic_paths = HashSet::new();
    
    let mut overlay_ids = HashSet::new();
    let mut hymo_ids = HashSet::new();
    let mut magic_ids = HashSet::new();

    let mut target_partitions = defs::BUILTIN_PARTITIONS.to_vec();
    target_partitions.extend(config.partitions.iter().map(|s| s.as_str()));

    for module in modules {
        let mut content_path = storage_root.join(&module.id);
        if !content_path.exists() {
            content_path = module.source_path.clone();
        }
        
        if !content_path.exists() { continue; }

        if let Ok(entries) = fs::read_dir(&content_path) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if !path.is_dir() { continue; }
                    
                    let dir_name = entry.file_name().to_string_lossy().to_string();
                    
                    if !target_partitions.contains(&dir_name.as_str()) {
                        continue;
                    }

                    if !has_files(&path) { continue; }

                    let mode = module.rules.get_mode(&dir_name);

                    match mode {
                        MountMode::Overlay => {
                            overlay_groups.entry(dir_name)
                                .or_default()
                                .push(path);
                            overlay_ids.insert(module.id.clone());
                        },
                        MountMode::HymoFs => {
                            let target_base = PathBuf::from("/").join(&dir_name);
                            plan.hymo_ops.push(HymoOperation {
                                module_id: module.id.clone(),
                                source: path,
                                target: target_base,
                            });
                            hymo_ids.insert(module.id.clone());
                        },
                        MountMode::Magic => {
                            magic_paths.insert(content_path.clone());
                            magic_ids.insert(module.id.clone());
                        },
                        MountMode::Ignore => {
                            log::debug!("Ignoring {}/{} per rule", module.id, dir_name);
                        }
                    }
                }
            }
        }
    }

    for (part, layers) in overlay_groups {
        let initial_target_path = format!("/{}", part);
        let target_path_obj = Path::new(&initial_target_path);
        
        let resolved_target = if target_path_obj.exists() {
             match target_path_obj.canonicalize() {
                 Ok(p) => p,
                 Err(_) => continue,
             }
        } else {
             continue;
        };

        if !resolved_target.is_dir() {
            continue;
        }

        plan.overlay_ops.push(OverlayOperation {
            partition_name: part,
            target: resolved_target.to_string_lossy().to_string(),
            lowerdirs: layers,
        });
    }

    plan.magic_module_paths = magic_paths.into_iter().collect();
    
    plan.overlay_module_ids = overlay_ids.into_iter().collect();
    plan.magic_module_ids = magic_ids.into_iter().collect();
    plan.hymo_module_ids = hymo_ids.into_iter().collect();
    
    plan.overlay_module_ids.sort();
    plan.magic_module_ids.sort();
    plan.hymo_module_ids.sort();

    Ok(plan)
}

fn has_files(path: &Path) -> bool {
    if let Ok(entries) = fs::read_dir(path) {
        for _ in entries.flatten() {
            return true;
        }
    }
    false
}
