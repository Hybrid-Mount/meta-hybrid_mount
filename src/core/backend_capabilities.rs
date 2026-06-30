// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use crate::conf::config::Config;
#[cfg(feature = "kasumi")]
use crate::sys::kasumi;

#[derive(Debug, Clone, Default)]
pub struct BackendCapabilities {
    kasumi_status: String,
    kasumi_usable: bool,
}

impl BackendCapabilities {
    pub fn detect(config: &Config) -> Self {
        #[cfg(not(feature = "kasumi"))]
        {
            let _ = config;
            Self {
                kasumi_status: "disabled".to_string(),
                kasumi_usable: false,
            }
        }

        #[cfg(feature = "kasumi")]
        {
            let status = kasumi::check_status();

            Self {
                kasumi_status: kasumi::status_name(status).to_string(),
                kasumi_usable: config.kasumi.enabled && kasumi::can_operate(),
            }
        }
    }

    pub fn can_use_kasumi(&self) -> bool {
        self.kasumi_usable
    }

    pub fn kasumi_status(&self) -> &str {
        &self.kasumi_status
    }
}
