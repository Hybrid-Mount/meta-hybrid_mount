// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

mod coordinator;
mod dir_walker;
mod module_processor;
mod plan_builder;
mod types;

#[cfg(test)]
mod tests;

// 公共 API
pub use coordinator::prepare_mount_plan;
