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
pub mod inventory;
#[cfg(feature = "kasumi")]
pub mod kasumi_coordinator;
pub mod module_status;
pub mod ops;
pub mod recovery;
pub mod runtime_finalization;
pub mod runtime_state;
pub mod startup;
pub mod storage;
#[cfg(feature = "kasumi")]
pub mod user_hide_rules;

pub use controller::MountController;
