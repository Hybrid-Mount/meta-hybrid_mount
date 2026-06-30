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

#[derive(Debug, Clone)]
#[cfg(feature = "kasumi")]
pub struct KasumiAddRule {
    pub target: String,
    pub source: PathBuf,
    pub file_type: i32,
}

#[derive(Debug, Clone)]
#[cfg(feature = "kasumi")]
pub struct KasumiMergeRule {
    pub target: String,
    pub source: PathBuf,
}

#[derive(Debug, Default)]
pub struct MountPlan {
    pub overlay_ops: Vec<OverlayOperation>,
    #[cfg(feature = "kasumi")]
    pub kasumi_add_rules: Vec<KasumiAddRule>,
    #[cfg(feature = "kasumi")]
    pub kasumi_merge_rules: Vec<KasumiMergeRule>,
    #[cfg(feature = "kasumi")]
    pub kasumi_hide_rules: Vec<String>,
    pub overlay_module_ids: Vec<String>,
    pub magic_module_ids: Vec<String>,
    #[cfg(feature = "kasumi")]
    pub kasumi_module_ids: Vec<String>,
}

impl MountPlan {
    pub fn kasumi_count(&self) -> usize {
        #[cfg(feature = "kasumi")]
        {
            self.kasumi_module_ids.len()
        }
        #[cfg(not(feature = "kasumi"))]
        {
            0
        }
    }
}
