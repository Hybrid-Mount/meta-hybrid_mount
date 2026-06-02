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

use std::{error::Error as StdError, fmt};

use anyhow::Error;

#[derive(Debug, Clone, Copy)]
pub enum FailureStage {
    Sync,
    Execute,
}

impl fmt::Display for FailureStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sync => write!(f, "sync"),
            Self::Execute => write!(f, "execute"),
        }
    }
}

#[derive(Debug)]
pub struct ModuleStageFailure {
    pub stage: FailureStage,
    pub module_ids: Vec<String>,
    pub source: Error,
}

impl ModuleStageFailure {
    pub fn new(stage: FailureStage, module_ids: Vec<String>, source: Error) -> Self {
        Self {
            stage,
            module_ids,
            source,
        }
    }

    pub fn sync(module_ids: Vec<String>, source: Error) -> Self {
        Self::new(FailureStage::Sync, module_ids, source)
    }

    pub fn execute(module_ids: Vec<String>, source: Error) -> Self {
        Self::new(FailureStage::Execute, module_ids, source)
    }

    pub fn sync_one(module_id: &str, source: Error) -> Self {
        Self::sync(vec![module_id.to_string()], source)
    }
}

impl fmt::Display for ModuleStageFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.module_ids.is_empty() {
            write!(
                f,
                "module stage failure during {}: {}",
                self.stage, self.source
            )
        } else {
            write!(
                f,
                "module stage failure during {} for [{}]: {}",
                self.stage,
                self.module_ids.join(", "),
                self.source
            )
        }
    }
}

impl StdError for ModuleStageFailure {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(self.source.as_ref())
    }
}
