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
