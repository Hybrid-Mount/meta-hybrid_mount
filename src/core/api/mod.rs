// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

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
