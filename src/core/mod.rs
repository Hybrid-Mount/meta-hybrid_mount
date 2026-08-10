// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

#[cfg(any(feature = "control-plane", feature = "kasumi"))]
pub mod api;
pub mod backend_capabilities;
#[cfg(feature = "control-plane")]
pub mod cli_commands;
pub mod controller;
#[cfg(feature = "control-plane")]
pub mod daemon;
#[cfg(feature = "control-plane")]
pub mod entry;
pub mod failure;
pub mod inventory;
#[cfg(feature = "kasumi")]
pub mod kasumi_coordinator;
pub mod late_load;
pub mod ops;
pub mod runtime_finalization;
pub mod runtime_state;
pub mod startup;
pub mod storage;
#[cfg(feature = "kasumi")]
pub mod user_hide_rules;

pub use controller::MountController;
