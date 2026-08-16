// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

mod client;
pub(crate) mod protocol;
mod server;

pub use self::{client::dispatch, server::serve};
