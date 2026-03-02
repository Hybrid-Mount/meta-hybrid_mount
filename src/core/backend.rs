// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{collections::HashSet, path::Path};

use anyhow::Result;

use crate::{conf::config::Config, core::ops::planner::OverlayOperation};

pub trait StorageBackend: Send + Sync {
    fn commit(&mut self, disable_umount: bool) -> Result<()>;
    fn mount_point(&self) -> &Path;
    fn mode(&self) -> &str;
}

pub trait MountDriver: Send + Sync {
    fn is_supported(&self) -> Result<bool>;
    fn mount_overlay(&self, op: &OverlayOperation, config: &Config) -> Result<Vec<String>>;
    fn mount_magic(
        &self,
        ids: &HashSet<String>,
        config: &Config,
        tempdir: &Path,
    ) -> Result<Vec<String>>;
}
