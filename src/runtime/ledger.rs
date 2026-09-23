// SPDX-License-Identifier: GPL-3.0-only

use super::rules::SavedRule;
use crate::errors::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct OwnedMount {
    pub target: String,
    pub id: i32,
    pub baseline_ids: Vec<i32>,
    pub device: String,
    pub fs_type: String,
    pub source: Option<String>,
}

/// Versioned, boot-scoped ownership. Missing fields are invalid, not an empty ledger.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Ledger {
    pub version: u32,
    pub boot_id: String,
    pub namespace: String,
    pub generation: u64,
    pub phase: String,
    pub rules: Vec<SavedRule>,
    pub pending_rules: Vec<SavedRule>,
    pub mounts: Vec<OwnedMount>,
    /// None identifies a pre-registration-ledger session; never infer its KSU ownership.
    #[serde(default)]
    pub ksu_unmounts: Option<Vec<String>>,
    pub non_vfs_modules: BTreeSet<String>,
    pub modules: Vec<crate::state::AppModule>,
    pub isolated_uids: Vec<u32>,
    pub error: Option<String>,
}

impl Ledger {
    pub fn require_clean(&self) -> Result<()> {
        if self.phase != "clean"
            || !self.pending_rules.is_empty()
            || !self.rules.is_empty()
            || !self.mounts.is_empty()
            || !self.isolated_uids.is_empty()
            || self
                .ksu_unmounts
                .as_ref()
                .is_some_and(|paths| !paths.is_empty())
        {
            return Err(Error::msg(
                "runtime resources already exist or need recovery; run soft-reboot cleanup before rebuilding",
            ));
        }
        Ok(())
    }

    pub fn require_ready(&self) -> Result<()> {
        if self.phase != "ready" {
            return Err(Error::msg(format!(
                "runtime is {}; complete recovery before hot operations",
                self.phase
            )));
        }
        Ok(())
    }

    pub fn owned_unmounts(&self) -> Result<&[String]> {
        match &self.ksu_unmounts {
            Some(paths) => Ok(paths),
            None if self.mounts.is_empty() => Ok(&[]),
            None => Err(Error::msg(
                "legacy runtime has no KernelSU registration ownership; full reboot required",
            )),
        }
    }

    pub fn release_mount_resources(
        &mut self,
        detach: impl FnOnce(&[OwnedMount]) -> Result<()>,
        release: impl FnOnce(&[String]) -> Result<()>,
    ) -> Result<()> {
        let registrations = self.owned_unmounts()?;
        detach(&self.mounts)?;
        release(registrations)?;
        // Keep both identities on any failure so a later cleanup can retry safely.
        self.mounts.clear();
        self.ksu_unmounts = Some(Vec::new());
        Ok(())
    }
}

pub fn ledger_for_boot(saved: Option<Ledger>, boot_id: &str, namespace: &str) -> Result<Ledger> {
    if let Some(saved) = saved
        && saved.boot_id == boot_id
    {
        if saved.namespace != namespace {
            return Err(Error::msg(
                "runtime mount namespace changed; refusing to reuse mount identities",
            ));
        }
        if saved.version != 1 {
            return Err(Error::msg("unsupported runtime ledger version"));
        }
        if !matches!(
            saved.phase.as_str(),
            "clean" | "applying" | "ready" | "syncing" | "cleaning" | "error"
        ) {
            return Err(Error::msg("invalid runtime lifecycle phase"));
        }
        return Ok(saved);
    }
    Ok(Ledger {
        version: 1,
        boot_id: boot_id.into(),
        namespace: namespace.into(),
        phase: "clean".into(),
        ksu_unmounts: Some(Vec::new()),
        ..Ledger::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn another_boot_never_reuses_owned_mounts() {
        let old = Ledger {
            boot_id: "old".into(),
            generation: 8,
            phase: "ready".into(),
            ..Ledger::default()
        };
        let fresh = ledger_for_boot(Some(old), "new", "ns").unwrap();
        assert_eq!(fresh.generation, 0);
        assert_eq!(fresh.phase, "clean");
    }

    #[test]
    fn corrupt_schema_is_not_treated_as_empty_state() {
        assert!(serde_json::from_str::<Ledger>("{}").is_err());
    }

    #[test]
    fn repeated_boot_apply_requires_cleanup() {
        let state = Ledger {
            boot_id: "boot".into(),
            phase: "ready".into(),
            ..Ledger::default()
        };
        assert!(state.require_clean().is_err());
    }

    #[test]
    fn pending_operation_cannot_be_reported_ready() {
        let state = Ledger {
            phase: "applying".into(),
            ..Ledger::default()
        };
        assert!(state.require_ready().is_err());
    }

    #[test]
    fn namespace_change_does_not_reuse_mount_ids() {
        let old = Ledger {
            boot_id: "boot".into(),
            namespace: "ns1".into(),
            ..Ledger::default()
        };
        assert!(ledger_for_boot(Some(old), "boot", "ns2").is_err());
    }

    #[test]
    fn cleanup_keeps_mount_ownership_when_ksu_release_fails() {
        let mut saved = Ledger {
            mounts: vec![OwnedMount {
                target: "/system/etc/hosts".into(),
                id: 17,
                ..OwnedMount::default()
            }],
            ksu_unmounts: Some(vec!["/system/etc/hosts".into()]),
            ..Ledger::default()
        };
        let mut detached = false;
        let result = saved.release_mount_resources(
            |_| {
                detached = true;
                Ok(())
            },
            |_| Err(Error::msg("ioctl failed")),
        );
        assert!(result.is_err());
        assert!(detached);
        assert_eq!(saved.mounts.len(), 1);
        assert_eq!(saved.owned_unmounts().unwrap(), ["/system/etc/hosts"]);
        saved
            .release_mount_resources(|_| Ok(()), |_| Ok(()))
            .unwrap();
        assert!(saved.mounts.is_empty());
        assert!(saved.owned_unmounts().unwrap().is_empty());
    }

    #[test]
    fn cleanup_does_not_release_registrations_before_mounts_are_detached() {
        let mut saved = Ledger {
            ksu_unmounts: Some(vec!["/system/etc/hosts".into()]),
            ..Ledger::default()
        };
        let result = saved.release_mount_resources(
            |_| Err(Error::msg("detach failed")),
            |_| panic!("must not remove hiding registrations while mount cleanup failed"),
        );
        assert!(result.is_err());
        assert_eq!(saved.owned_unmounts().unwrap().len(), 1);
    }

    #[test]
    fn legacy_real_mounts_require_reboot_instead_of_guessing_ksu_ownership() {
        let old = Ledger {
            mounts: vec![OwnedMount::default()],
            ..Ledger::default()
        };
        assert!(old.owned_unmounts().is_err());
        assert!(Ledger::default().owned_unmounts().unwrap().is_empty());
    }

    #[test]
    fn clean_phase_with_pending_ksu_registration_cannot_rebuild() {
        let saved = Ledger {
            phase: "clean".into(),
            ksu_unmounts: Some(vec!["/system/etc/hosts".into()]),
            ..Ledger::default()
        };
        assert!(saved.require_clean().is_err());
    }
}

pub const LEDGER_PATH: &str = "/data/adb/hybrid-mount/run/runtime.json";
const LOCK_PATH: &str = "/data/adb/hybrid-mount/run/runtime.lock";

/// Never unlink this file: waiters must keep locking the same inode.
pub struct OperationLock(std::fs::File);

impl OperationLock {
    pub fn acquire() -> Result<Self> {
        Self::at(std::path::Path::new(LOCK_PATH))
    }

    fn at(path: &std::path::Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut options = std::fs::OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(path)?;
        file.try_lock().map_err(|err| {
            Error::msg(format!(
                "another runtime operation is active or locking failed: {err}"
            ))
        })?;
        Ok(Self(file))
    }
}

impl Drop for OperationLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

pub fn read_at(path: &std::path::Path) -> Result<Option<Ledger>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.into()),
    }
}

pub fn save(ledger: &Ledger) -> Result<()> {
    crate::sys::fs::atomic_write(
        std::path::Path::new(LEDGER_PATH),
        &serde_json::to_vec_pretty(ledger)?,
    )
}

#[cfg(test)]
mod lock_tests {
    use super::*;
    #[test]
    fn lock_serializes_and_releases_when_owner_exits() {
        let temp = crate::test_support::Fixture::new("runtime-lock");
        let path = temp.join("lock");
        let lock = OperationLock::at(&path).unwrap();
        assert!(OperationLock::at(&path).is_err());
        drop(lock);
        assert!(OperationLock::at(&path).is_ok());
    }
}
