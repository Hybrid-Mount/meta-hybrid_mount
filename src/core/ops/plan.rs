// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::path::PathBuf;

#[derive(Debug, Default, Clone, Copy)]
pub struct PrepareMetrics {
    pub elapsed_ms: u64,
    pub directories_scanned: usize,
    pub entries_scanned: usize,
    pub copied_entries: usize,
    pub copied_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct OverlayOperation {
    pub partition_name: String,
    pub target: String,
    pub lowerdirs: Vec<PathBuf>,
}

#[derive(Debug, Default)]
pub struct MountPlan {
    pub prepare_metrics: PrepareMetrics,
    pub overlay_ops: Vec<OverlayOperation>,
    pub overlay_module_ids: Vec<String>,
    pub magic_module_ids: Vec<String>,
}
