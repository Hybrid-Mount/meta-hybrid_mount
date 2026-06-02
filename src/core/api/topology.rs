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

#[cfg(any(target_os = "linux", target_os = "android"))]
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

#[cfg(any(target_os = "linux", target_os = "android"))]
use anyhow::{Context, Result};
#[cfg(any(target_os = "linux", target_os = "android"))]
use procfs::process::{MountInfo, MountOptFields, Process};
use serde::Serialize;

#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::defs;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::partitions;
use crate::{conf::config::Config, core::runtime_state::RuntimeState};

#[derive(Debug, Clone, Serialize)]
pub struct MountTopologyPayload {
    pub supported: bool,
    pub inspected_pid: u32,
    pub state_pid: u32,
    pub configured_mount_source: String,
    pub state_mount_point: String,
    pub active_mounts: Vec<String>,
    pub error: Option<String>,
    pub summary: Option<MountTopologySummary>,
    pub warnings: Vec<String>,
    pub focus_mounts: Vec<MountTopologyEntry>,
    pub shared_peer_groups: Vec<SharedPeerGroupSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MountTopologySummary {
    pub total_mounts: usize,
    pub kasumi_excluded_mounts: usize,
    pub inspected_mounts: usize,
    pub focus_mounts: usize,
    pub project_related_mounts: usize,
    pub managed_partition_root_mounts: usize,
    pub managed_partition_root_propagation_mounts: usize,
    pub active_partition_tree_mounts: usize,
    pub active_partition_tree_propagation_mounts: usize,
    pub hybrid_mount_internal_mounts: usize,
    pub hybrid_mount_internal_propagation_mounts: usize,
    pub storage_mounts: usize,
    pub overlayfs_mounts: usize,
    pub shared_mounts: usize,
    pub slave_mounts: usize,
    pub receiving_propagation_mounts: usize,
    pub propagate_from_mounts: usize,
    pub unbindable_mounts: usize,
    pub shared_peer_groups: usize,
    pub largest_shared_peer_group: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct MountTopologyEntry {
    pub mount_id: i32,
    pub parent_mount_id: i32,
    pub major_minor: String,
    pub mount_point: String,
    pub root: String,
    pub fs_type: String,
    pub mount_source: Option<String>,
    pub propagation: MountPropagationInfo,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct MountPropagationInfo {
    pub shared: Option<u32>,
    pub master: Option<u32>,
    pub propagate_from: Option<u32>,
    pub unbindable: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SharedPeerGroupSummary {
    pub peer_group: u32,
    pub mount_count: usize,
    pub managed_partition_root_mounts: usize,
    pub active_partition_tree_mounts: usize,
    pub hybrid_mount_internal_mounts: usize,
    pub overlayfs_mounts: usize,
    pub mount_points: Vec<String>,
}

impl MountTopologyPayload {
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    pub fn unsupported(msg: &str) -> Self {
        Self {
            supported: false,
            inspected_pid: std::process::id(),
            state_pid: 0,
            configured_mount_source: String::new(),
            state_mount_point: String::new(),
            active_mounts: Vec::new(),
            error: Some(msg.to_string()),
            summary: None,
            warnings: Vec::new(),
            focus_mounts: Vec::new(),
            shared_peer_groups: Vec::new(),
        }
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    fn from_error(
        state: &RuntimeState,
        config: &Config,
        inspected_pid: u32,
        err_msg: &str,
    ) -> Self {
        Self {
            supported: true,
            inspected_pid,
            state_pid: state.pid,
            configured_mount_source: config.mountsource.clone(),
            state_mount_point: state.mount_point.display().to_string(),
            active_mounts: state.active_mounts.clone(),
            error: Some(err_msg.to_string()),
            summary: None,
            warnings: Vec::new(),
            focus_mounts: Vec::new(),
            shared_peer_groups: Vec::new(),
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[derive(Debug, Default)]
struct MountCounters {
    total_mounts: usize,
    kasumi_excluded_mounts: usize,
    project_related_mounts: usize,
    managed_partition_root_mounts: usize,
    managed_partition_root_propagation_mounts: usize,
    active_partition_tree_mounts: usize,
    active_partition_tree_propagation_mounts: usize,
    hybrid_mount_internal_mounts: usize,
    hybrid_mount_internal_propagation_mounts: usize,
    storage_mounts: usize,
    overlayfs_mounts: usize,
    shared_mounts: usize,
    slave_mounts: usize,
    receiving_propagation_mounts: usize,
    propagate_from_mounts: usize,
    unbindable_mounts: usize,
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[derive(Debug, Default)]
struct PeerGroupAccumulator {
    mount_count: usize,
    managed_partition_root_mounts: usize,
    active_partition_tree_mounts: usize,
    hybrid_mount_internal_mounts: usize,
    overlayfs_mounts: usize,
    mount_points: Vec<String>,
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[derive(Debug, Default)]
struct MountClassifications {
    tags: Vec<String>,
    project_related: bool,
    managed_partition_root: bool,
    active_partition_tree: bool,
    hybrid_mount_internal: bool,
    storage_mount: bool,
    overlayfs: bool,
}

pub fn build_mount_topology_payload(config: &Config, state: &RuntimeState) -> MountTopologyPayload {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        let inspected_pid = std::process::id();

        match collect_mount_topology(config, state, inspected_pid) {
            Ok(payload) => payload,
            Err(err) => {
                MountTopologyPayload::from_error(state, config, inspected_pid, &format!("{err:#}"))
            }
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        let _ = (config, state);
        MountTopologyPayload::unsupported(
            "mount topology inspection is only supported on linux/android",
        )
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn collect_mount_topology(
    config: &Config,
    state: &RuntimeState,
    inspected_pid: u32,
) -> Result<MountTopologyPayload> {
    let managed_partition_roots = partitions::managed_partition_names()
        .into_iter()
        .map(|name| PathBuf::from(format!("/{name}")))
        .collect::<Vec<_>>();
    let active_partition_roots = state
        .active_mounts
        .iter()
        .filter(|name| name.as_str() != "kasumi")
        .map(|name| PathBuf::from(format!("/{name}")))
        .collect::<Vec<_>>();

    let mountinfo = Process::myself()
        .context("failed to open self procfs handle")?
        .mountinfo()
        .context("failed to read mountinfo")?;

    let mut counters = MountCounters::default();
    let mut focus_mounts = Vec::new();
    let mut peer_groups: BTreeMap<u32, PeerGroupAccumulator> = BTreeMap::new();

    for mount in mountinfo {
        counters.total_mounts += 1;

        if is_kasumi_mount(&mount, config) {
            counters.kasumi_excluded_mounts += 1;
            continue;
        }

        let propagation = collect_propagation(&mount.opt_fields);
        let classifications = classify_mount(
            &mount,
            &propagation,
            &managed_partition_roots,
            &active_partition_roots,
            state.mount_point.as_path(),
        );
        let has_propagation = propagation.shared.is_some()
            || propagation.master.is_some()
            || propagation.propagate_from.is_some()
            || propagation.unbindable;

        if classifications.project_related {
            counters.project_related_mounts += 1;
        }
        if classifications.managed_partition_root {
            counters.managed_partition_root_mounts += 1;
            if has_propagation {
                counters.managed_partition_root_propagation_mounts += 1;
            }
        }
        if classifications.active_partition_tree {
            counters.active_partition_tree_mounts += 1;
            if has_propagation {
                counters.active_partition_tree_propagation_mounts += 1;
            }
        }
        if classifications.hybrid_mount_internal {
            counters.hybrid_mount_internal_mounts += 1;
            if has_propagation {
                counters.hybrid_mount_internal_propagation_mounts += 1;
            }
        }
        if classifications.storage_mount {
            counters.storage_mounts += 1;
        }
        if classifications.overlayfs {
            counters.overlayfs_mounts += 1;
        }
        if propagation.shared.is_some() {
            counters.shared_mounts += 1;
        }
        if propagation.master.is_some() {
            counters.slave_mounts += 1;
        }
        if propagation.master.is_some() || propagation.propagate_from.is_some() {
            counters.receiving_propagation_mounts += 1;
        }
        if propagation.propagate_from.is_some() {
            counters.propagate_from_mounts += 1;
        }
        if propagation.unbindable {
            counters.unbindable_mounts += 1;
        }

        if let Some(peer_group) = propagation.shared {
            let group = peer_groups.entry(peer_group).or_default();
            group.mount_count += 1;
            group.managed_partition_root_mounts +=
                usize::from(classifications.managed_partition_root);
            group.active_partition_tree_mounts +=
                usize::from(classifications.active_partition_tree);
            group.hybrid_mount_internal_mounts +=
                usize::from(classifications.hybrid_mount_internal);
            group.overlayfs_mounts += usize::from(classifications.overlayfs);
            group
                .mount_points
                .push(mount.mount_point.display().to_string());
        }

        if should_include_focus_mount(&classifications, &propagation) {
            focus_mounts.push(MountTopologyEntry {
                mount_id: mount.mnt_id,
                parent_mount_id: mount.pid,
                major_minor: mount.majmin.clone(),
                mount_point: mount.mount_point.display().to_string(),
                root: mount.root.clone(),
                fs_type: mount.fs_type.clone(),
                mount_source: mount.mount_source.clone(),
                propagation,
                tags: classifications.tags,
            });
        }
    }

    focus_mounts.sort_by(|left, right| {
        left.mount_point
            .cmp(&right.mount_point)
            .then(left.mount_id.cmp(&right.mount_id))
    });

    let shared_peer_groups = peer_groups
        .into_iter()
        .map(|(peer_group, mut group)| {
            group.mount_points.sort();
            group.mount_points.dedup();
            if group.mount_points.len() > 8 {
                group.mount_points.truncate(8);
            }

            SharedPeerGroupSummary {
                peer_group,
                mount_count: group.mount_count,
                managed_partition_root_mounts: group.managed_partition_root_mounts,
                active_partition_tree_mounts: group.active_partition_tree_mounts,
                hybrid_mount_internal_mounts: group.hybrid_mount_internal_mounts,
                overlayfs_mounts: group.overlayfs_mounts,
                mount_points: group.mount_points,
            }
        })
        .collect::<Vec<_>>();

    let largest_shared_peer_group = shared_peer_groups
        .iter()
        .map(|group| group.mount_count)
        .max()
        .unwrap_or(0);

    let inspected_mounts = counters
        .total_mounts
        .saturating_sub(counters.kasumi_excluded_mounts);

    let warnings = build_warnings(&counters, &shared_peer_groups);

    Ok(MountTopologyPayload {
        supported: true,
        inspected_pid,
        state_pid: state.pid,
        configured_mount_source: config.mountsource.clone(),
        state_mount_point: state.mount_point.display().to_string(),
        active_mounts: state.active_mounts.clone(),
        error: None,
        summary: Some(MountTopologySummary {
            total_mounts: counters.total_mounts,
            kasumi_excluded_mounts: counters.kasumi_excluded_mounts,
            inspected_mounts,
            focus_mounts: focus_mounts.len(),
            project_related_mounts: counters.project_related_mounts,
            managed_partition_root_mounts: counters.managed_partition_root_mounts,
            managed_partition_root_propagation_mounts: counters
                .managed_partition_root_propagation_mounts,
            active_partition_tree_mounts: counters.active_partition_tree_mounts,
            active_partition_tree_propagation_mounts: counters
                .active_partition_tree_propagation_mounts,
            hybrid_mount_internal_mounts: counters.hybrid_mount_internal_mounts,
            hybrid_mount_internal_propagation_mounts: counters
                .hybrid_mount_internal_propagation_mounts,
            storage_mounts: counters.storage_mounts,
            overlayfs_mounts: counters.overlayfs_mounts,
            shared_mounts: counters.shared_mounts,
            slave_mounts: counters.slave_mounts,
            receiving_propagation_mounts: counters.receiving_propagation_mounts,
            propagate_from_mounts: counters.propagate_from_mounts,
            unbindable_mounts: counters.unbindable_mounts,
            shared_peer_groups: shared_peer_groups.len(),
            largest_shared_peer_group,
        }),
        warnings,
        focus_mounts,
        shared_peer_groups,
    })
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn collect_propagation(fields: &[MountOptFields]) -> MountPropagationInfo {
    let mut info = MountPropagationInfo::default();

    for field in fields {
        match field {
            MountOptFields::Shared(value) => info.shared = Some(*value),
            MountOptFields::Master(value) => info.master = Some(*value),
            MountOptFields::PropagateFrom(value) => info.propagate_from = Some(*value),
            MountOptFields::Unbindable => info.unbindable = true,
        }
    }

    info
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn classify_mount(
    mount: &MountInfo,
    propagation: &MountPropagationInfo,
    managed_partition_roots: &[PathBuf],
    active_partition_roots: &[PathBuf],
    state_mount_point: &Path,
) -> MountClassifications {
    let mut tags = BTreeSet::new();
    let mount_point = mount.mount_point.as_path();
    let mount_source = mount.mount_source.as_deref().map(Path::new);

    let managed_partition_root = managed_partition_roots
        .iter()
        .any(|root| mount_point == root);
    let active_partition_tree = active_partition_roots
        .iter()
        .any(|root| mount_point.starts_with(root));
    let hybrid_mount_internal = mount_point.starts_with(defs::HYBRID_MOUNT_DIR)
        || mount_source.is_some_and(|path| path.starts_with(defs::HYBRID_MOUNT_DIR));
    let storage_mount = !state_mount_point.as_os_str().is_empty()
        && (mount_point == state_mount_point
            || mount_point.starts_with(state_mount_point)
            || mount_source.is_some_and(|path| {
                path == state_mount_point || path.starts_with(state_mount_point)
            }));
    let overlayfs = mount.fs_type == "overlay";

    if managed_partition_root {
        tags.insert("managed_partition_root".to_string());
    }
    if active_partition_tree {
        tags.insert("active_partition_tree".to_string());
    }
    if hybrid_mount_internal {
        tags.insert("hybrid_mount_internal".to_string());
    }
    if storage_mount {
        tags.insert("mount_storage".to_string());
    }
    if overlayfs {
        tags.insert("overlayfs".to_string());
    }
    if propagation.shared.is_some() {
        tags.insert("shared_peer".to_string());
    }
    if propagation.master.is_some() {
        tags.insert("slave".to_string());
    }
    if propagation.propagate_from.is_some() {
        tags.insert("propagate_from".to_string());
    }
    if propagation.unbindable {
        tags.insert("unbindable".to_string());
    }

    MountClassifications {
        project_related: managed_partition_root
            || active_partition_tree
            || hybrid_mount_internal
            || storage_mount,
        managed_partition_root,
        active_partition_tree,
        hybrid_mount_internal,
        storage_mount,
        overlayfs,
        tags: tags.into_iter().collect(),
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn should_include_focus_mount(
    classifications: &MountClassifications,
    propagation: &MountPropagationInfo,
) -> bool {
    classifications.project_related
        || classifications.overlayfs
        || propagation.shared.is_some()
        || propagation.master.is_some()
        || propagation.propagate_from.is_some()
        || propagation.unbindable
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn is_kasumi_mount(mount: &MountInfo, config: &Config) -> bool {
    let mount_point = mount.mount_point.as_path();
    let mount_source = mount.mount_source.as_deref().map(Path::new);

    mount.fs_type.to_ascii_lowercase().contains("kasumi")
        || mount_point.starts_with(config.kasumi.mirror_path.as_path())
        || mount_source.is_some_and(|path| path.starts_with(config.kasumi.mirror_path.as_path()))
        || mount_point.starts_with(defs::KASUMI_MIRROR_DIR)
        || mount_source.is_some_and(|path| path.starts_with(defs::KASUMI_MIRROR_DIR))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn build_warnings(
    counters: &MountCounters,
    shared_peer_groups: &[SharedPeerGroupSummary],
) -> Vec<String> {
    let mut warnings = Vec::new();

    if counters.shared_mounts > 0 {
        warnings.push(format!(
            "{} non-Kasumi mounts still belong to shared peer groups.",
            counters.shared_mounts
        ));
    }
    if counters.receiving_propagation_mounts > 0 {
        warnings.push(format!(
            "{} non-Kasumi mounts still receive propagation from another peer group.",
            counters.receiving_propagation_mounts
        ));
    }
    if counters.hybrid_mount_internal_propagation_mounts > 0 {
        warnings.push(format!(
            "{} Hybrid Mount internal mounts still show propagation metadata; compare these before and after setup when users report mount peer gaps.",
            counters.hybrid_mount_internal_propagation_mounts
        ));
    }
    if counters.managed_partition_root_propagation_mounts > 0 {
        warnings.push(format!(
            "{} managed partition root mounts still show propagation metadata.",
            counters.managed_partition_root_propagation_mounts
        ));
    }
    if counters.active_partition_tree_propagation_mounts > 0 {
        warnings.push(format!(
            "{} mounts under active partition trees still show propagation metadata; these are the first candidates to diff in issue reports.",
            counters.active_partition_tree_propagation_mounts
        ));
    }
    if let Some(group) = shared_peer_groups
        .iter()
        .max_by_key(|group| group.mount_count)
    {
        warnings.push(format!(
            "Largest non-Kasumi shared peer group is {} with {} mounts.",
            group.peer_group, group.mount_count
        ));
    }

    warnings
}
