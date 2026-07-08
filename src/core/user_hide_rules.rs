// Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use crate::{
    defs,
    sys::{fs::atomic_write, kasumi},
};

fn load_user_hide_rules_from(path: &Path) -> Result<Vec<PathBuf>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read user hide rules file {}", path.display()))?;
    let values: Vec<String> = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse user hide rules file {}", path.display()))?;
    Ok(values.into_iter().map(PathBuf::from).collect())
}

fn save_user_hide_rules_to(path: &Path, rules: &[PathBuf]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create user hide rules parent directory {}",
                parent.display()
            )
        })?;
    }

    let values: Vec<String> = rules
        .iter()
        .map(|rule| rule.to_string_lossy().into_owned())
        .collect();
    let payload =
        serde_json::to_string_pretty(&values).context("failed to serialize user hide rules")?;
    atomic_write(path, payload.as_bytes())
        .with_context(|| format!("failed to write user hide rules file {}", path.display()))?;
    Ok(())
}

pub fn load_user_hide_rules() -> Result<Vec<PathBuf>> {
    load_user_hide_rules_from(Path::new(defs::USER_HIDE_RULES_FILE))
}

pub fn save_user_hide_rules(rules: &[PathBuf]) -> Result<()> {
    save_user_hide_rules_to(Path::new(defs::USER_HIDE_RULES_FILE), rules)
}

pub fn user_hide_rule_count() -> usize {
    load_user_hide_rules().map(|rules| rules.len()).unwrap_or(0)
}

pub fn add_user_hide_rule(path: &Path) -> Result<bool> {
    if !path.is_absolute() {
        bail!("hide path must be absolute: {}", path.display());
    }

    let mut rules = load_user_hide_rules()?;
    if rules.iter().any(|rule| rule == path) {
        return Ok(false);
    }

    rules.push(path.to_path_buf());
    save_user_hide_rules(&rules)?;
    Ok(true)
}

pub fn remove_user_hide_rule(path: &Path) -> Result<bool> {
    let mut rules = load_user_hide_rules()?;
    let previous_len = rules.len();
    rules.retain(|rule| rule != path);

    if rules.len() == previous_len {
        return Ok(false);
    }

    save_user_hide_rules(&rules)?;
    Ok(true)
}

pub fn apply_user_hide_rules() -> Result<(usize, usize)> {
    let rules = load_user_hide_rules()?;
    apply_user_hide_rules_from_paths(&rules)
}

pub fn apply_user_hide_rules_from_paths(rules: &[PathBuf]) -> Result<(usize, usize)> {
    if rules.is_empty() {
        return Ok((0, 0));
    }

    let mut success = 0usize;
    let mut failed = 0usize;

    for path in rules {
        match kasumi::hide_path(path) {
            Ok(()) => success += 1,
            Err(err) => {
                failed += 1;
                crate::scoped_log!(
                    warn,
                    "user-hide-rules",
                    "apply failed: path={}, error={:#}",
                    path.display(),
                    err
                );
            }
        }
    }

    Ok((success, failed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_user_hide_rules_round_trips_paths() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("user_hide_rules.json");
        let rules = vec![
            PathBuf::from("/system/bin/example"),
            PathBuf::from("/vendor/lib64/libexample.so"),
        ];

        save_user_hide_rules_to(&path, &rules).unwrap();

        assert_eq!(load_user_hide_rules_from(&path).unwrap(), rules);
    }

    #[test]
    fn load_user_hide_rules_returns_empty_when_missing() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("missing.json");

        assert!(load_user_hide_rules_from(&path).unwrap().is_empty());
    }
}
