// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

#[cfg(feature = "kasumi")]
use std::collections::HashSet;
use std::{fs, path::Path};

use crate::defs;

const SYSTEM_PARTITION: &str = "system";

fn partition_root_exists(name: &str) -> bool {
    fs::symlink_metadata(Path::new("/").join(name)).is_ok()
}

pub fn managed_partition_names() -> Vec<String> {
    crate::scoped_log!(
        debug,
        "partitions:discover",
        "start: managed_candidates={}",
        defs::MANAGED_PARTITIONS.len() + 1,
    );

    let mut names = [SYSTEM_PARTITION]
        .into_iter()
        .chain(defs::MANAGED_PARTITIONS.iter().copied())
        .filter(|partition| partition_root_exists(partition))
        .map(str::to_string)
        .collect::<Vec<_>>();

    names.sort();
    names.dedup();

    crate::scoped_log!(
        debug,
        "partitions:discover",
        "complete: discovered={}",
        names.len()
    );

    names
}

#[cfg(feature = "kasumi")]
pub fn managed_partition_set() -> HashSet<String> {
    managed_partition_names().into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_keep_existing_root_partitions() {
        let partitions = managed_partition_names();

        for name in &partitions {
            assert!(partition_root_exists(name));
        }
    }
}
