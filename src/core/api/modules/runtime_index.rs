// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::HashSet;

use crate::{core::runtime_state::RuntimeState, domain::MountMode};

pub(super) struct RuntimeModuleIndex<'a> {
    overlay: HashSet<&'a str>,
    magic: HashSet<&'a str>,
    skipped: HashSet<&'a str>,
    blacklisted: HashSet<&'a str>,
}

impl<'a> RuntimeModuleIndex<'a> {
    pub(super) fn new(state: &'a RuntimeState) -> Self {
        Self {
            overlay: state.overlay_modules.iter().map(String::as_str).collect(),
            magic: state.magic_modules.iter().map(String::as_str).collect(),
            skipped: state
                .skip_mount_modules
                .iter()
                .map(String::as_str)
                .collect(),
            blacklisted: state
                .blacklisted_modules
                .iter()
                .map(String::as_str)
                .collect(),
        }
    }

    pub(super) fn mode(&self, module_id: &str) -> Option<MountMode> {
        [
            (&self.overlay, MountMode::Overlay),
            (&self.magic, MountMode::Magic),
        ]
        .into_iter()
        .find(|(set, _)| set.contains(module_id))
        .map(|(_, mode)| mode)
    }

    pub(super) fn enabled(&self, module_id: &str) -> bool {
        !self.skipped.contains(module_id) && !self.blacklisted.contains(module_id)
    }

    pub(super) fn is_blacklisted(&self, module_id: &str) -> bool {
        self.blacklisted.contains(module_id)
    }
}
