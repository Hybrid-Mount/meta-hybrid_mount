// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use crate::errors::Error;
use crate::vfs::protocol::EncodedRule;
use std::cell::Cell;

struct MockKernel<'a> {
    before: Option<&'static str>,
    after: Option<&'static str>,
    loaded: &'a Cell<bool>,
    applied: usize,
}

impl VfsKernel for MockKernel<'_> {
    fn version(&mut self) -> Result<String> {
        let value = if self.loaded.get() {
            self.after
        } else {
            self.before
        };
        value.map(str::to_owned).ok_or(Error::VfsUnavailable {
            reason: "absent".to_owned(),
        })
    }

    fn apply_rules(&mut self, rules: &[EncodedRule]) -> Result<()> {
        self.applied += rules.len();
        Ok(())
    }

    fn add_uids(&mut self, _uids: &[u32]) -> Result<()> {
        Ok(())
    }

    fn clear_rules(&mut self) -> Result<()> {
        Ok(())
    }
}

struct MockLoader<'a> {
    loaded: &'a Cell<bool>,
    calls: Cell<usize>,
}

impl LkmLoader for MockLoader<'_> {
    fn load_hm_vfs(&self) -> Result<()> {
        self.calls.set(self.calls.get() + 1);
        self.loaded.set(true);
        Ok(())
    }
}

#[test]
fn existing_supported_provider_is_adopted_without_loading() {
    let loaded = Cell::new(true);
    let mut kernel = MockKernel {
        before: Some("20"),
        after: Some("20"),
        loaded: &loaded,
        applied: 0,
    };
    let loader = MockLoader {
        loaded: &loaded,
        calls: Cell::new(0),
    };

    let selected = select_provider(&mut kernel, &loader, SUPPORTED_VERSIONS, false).unwrap();

    assert_eq!(selected, Some(VfsProvider::Nomount));
    assert_eq!(loader.calls.get(), 0);
}

#[test]
fn missing_provider_loads_hm_lkm_then_reports_hm() {
    let loaded = Cell::new(false);
    let mut kernel = MockKernel {
        before: None,
        after: Some("20"),
        loaded: &loaded,
        applied: 0,
    };
    let loader = MockLoader {
        loaded: &loaded,
        calls: Cell::new(0),
    };

    let selected = select_provider(&mut kernel, &loader, SUPPORTED_VERSIONS, false).unwrap();

    assert_eq!(selected, Some(VfsProvider::Hm));
    assert_eq!(loader.calls.get(), 1);
}

#[test]
fn missing_provider_stays_unavailable_when_lkm_does_not_help() {
    let loaded = Cell::new(false);
    let mut kernel = MockKernel {
        before: None,
        after: None,
        loaded: &loaded,
        applied: 0,
    };
    let loader = MockLoader {
        loaded: &loaded,
        calls: Cell::new(0),
    };

    let selected = select_provider(&mut kernel, &loader, SUPPORTED_VERSIONS, false).unwrap();

    assert_eq!(selected, None);
    assert_eq!(loader.calls.get(), 1);
}

#[test]
fn unsupported_existing_version_is_rejected_without_loading() {
    let loaded = Cell::new(false);
    let mut kernel = MockKernel {
        before: Some("19"),
        after: Some("20"),
        loaded: &loaded,
        applied: 0,
    };
    let loader = MockLoader {
        loaded: &loaded,
        calls: Cell::new(0),
    };

    let err = select_provider(&mut kernel, &loader, SUPPORTED_VERSIONS, false).unwrap_err();

    assert!(matches!(err, Error::VfsUnsupportedVersion { .. }));
    assert_eq!(loader.calls.get(), 0);
}
