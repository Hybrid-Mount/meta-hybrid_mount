// Copyright 2026 Hybrid Mount Developers
// SPDX-License-Identifier: GPL-3.0-or-later

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
