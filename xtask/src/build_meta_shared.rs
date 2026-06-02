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

use std::process::Command;

use anyhow::{Context, Result, bail};
use semver::Version;
use serde::Deserialize;

#[allow(dead_code)]
#[path = "../../src/defs.rs"]
pub mod defs;

#[derive(Deserialize)]
pub struct CargoConfig {
    pub package: Package,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct Package {
    pub authors: Vec<String>,
    pub name: String,
    pub version: String,
    pub description: String,
    pub metadata: PackageMetadata,
}

#[derive(Deserialize)]
pub struct PackageMetadata {
    pub hybrid_mount: HybridMountMetadata,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct HybridMountMetadata {
    pub name: String,
    pub update: String,
    pub lite_name: Option<String>,
    pub lite_update: Option<String>,
    pub nano_name: Option<String>,
    pub nano_update: Option<String>,
}

pub struct ModulePropData<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub version: &'a str,
    pub version_code: &'a str,
    pub author: &'a str,
    pub description: &'a str,
    pub update_json: &'a str,
    pub webui_icon: bool,
}

pub fn calculate_version_code(version_str: &str) -> Result<String> {
    let version = Version::parse(version_str)?;
    Ok((version.major * 100000 + version.minor * 1000 + version.patch).to_string())
}

pub fn git_commit_count() -> Result<i32> {
    let output = Command::new("git")
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .context("failed to run git rev-list")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git rev-list --count HEAD failed: {}", stderr.trim());
    }

    let stdout = String::from_utf8(output.stdout)?;
    stdout
        .trim()
        .parse::<i32>()
        .with_context(|| format!("invalid git commit count: {stdout:?}"))
}

pub fn render_module_prop(data: &ModulePropData<'_>) -> String {
    let mut content = format!(
        "id={}\nname={}\nversion={}\nversionCode={}\nauthor={}\ndescription={}\nupdateJson={}\nmetamodule=1\n",
        data.id,
        data.name,
        data.version,
        data.version_code,
        data.author,
        data.description,
        data.update_json,
    );
    if data.webui_icon {
        content.push_str("webuiIcon=launcher.png\n");
    }
    content
}

#[allow(dead_code)]
pub fn render_webui_constants(
    version: &str,
    is_release: bool,
    enable_kasumi: bool,
    config_path: &str,
    state_path: &str,
    binary_path: &str,
) -> String {
    format!(
        "export const APP_VERSION = \"{version}\";\nexport const IS_RELEASE = {is_release};\nexport const ENABLE_KASUMI = {enable_kasumi};\nexport const RUST_PATHS = {{\n  CONFIG: \"{config_path}\",\n  DAEMON_STATE: \"{state_path}\",\n  BINARY: \"{binary_path}\",\n}} as const;\n"
    )
}
