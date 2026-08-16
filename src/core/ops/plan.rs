// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct OverlayOperation {
    pub partition_name: String,
    pub target: String,
    pub lowerdirs: Vec<PathBuf>,
}

#[derive(Debug, Default)]
pub struct MountPlan {
    pub overlay_ops: Vec<OverlayOperation>,
    pub overlay_module_ids: Vec<String>,
    pub magic_module_ids: Vec<String>,
}
