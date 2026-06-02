// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

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
