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

mod error;
#[cfg(feature = "kasumi")]
mod kasumi;
mod modules;
mod system;
mod topology;

#[cfg(feature = "kasumi")]
pub use self::kasumi::{
    build_features_payload, build_kasumi_version_payload, build_lkm_payload,
    parse_kasumi_rule_listing,
};
pub use self::{
    error::print_json_error,
    modules::{
        ModuleApplyEntry, apply_modules_payload, build_modules_payload, build_version_payload,
    },
    system::{
        build_mount_stats_payload, build_partitions_payload, build_storage_payload,
        build_system_info_payload,
    },
    topology::build_mount_topology_payload,
};
