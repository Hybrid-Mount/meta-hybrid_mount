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

use std::path::Path;

#[cfg(any(target_os = "linux", target_os = "android"))]
use ksu::NukeExt4Sysfs;

pub fn nuke_path(path: &Path) {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        if !crate::utils::KSU.load(std::sync::atomic::Ordering::Relaxed) {
            crate::scoped_log!(
                debug,
                "nuke",
                "execute skipped: path={}, reason=non_ksu",
                path.display()
            );
            return;
        }

        let mut nuke = NukeExt4Sysfs::new();
        nuke.add(path);
        if let Err(e) = nuke.execute() {
            crate::scoped_log!(
                warn,
                "nuke",
                "execute failed: path={}, error={:#}",
                path.display(),
                e
            );
        } else {
            crate::scoped_log!(debug, "nuke", "execute success: path={}", path.display());
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    let _ = path;
}
