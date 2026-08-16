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

#[derive(Debug, Default)]
pub struct MountPlan {
    pub overlay_ops: Vec<OverlayOperation>,
    pub overlay_module_ids: Vec<String>,
    pub magic_module_ids: Vec<String>,
}
