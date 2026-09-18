// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use crate::errors::Error;
use crate::vfs::protocol::EncodedRule;

struct MockKernel {
    version: Option<&'static str>,
    applied: usize,
}

impl VfsKernel for MockKernel {
    fn version(&mut self) -> Result<String> {
        self.version
            .map(str::to_owned)
            .ok_or(Error::VfsUnavailable {
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

    fn remove_rules(&mut self, rules: &[EncodedRule]) -> Result<()> {
        self.applied = self.applied.saturating_sub(rules.len());
        Ok(())
    }
}

#[test]
fn existing_k2_is_adopted() {
    let mut kernel = MockKernel {
        version: Some("hm1"),
        applied: 0,
    };

    let selected = select_provider(&mut kernel, SUPPORTED_VERSIONS, false).unwrap();

    assert_eq!(selected, Some(VfsProvider::Hm));
}

#[test]
fn missing_provider_stays_unavailable_without_distributed_lkm() {
    let mut kernel = MockKernel {
        version: None,
        applied: 0,
    };

    let selected = select_provider(&mut kernel, SUPPORTED_VERSIONS, false).unwrap();

    assert_eq!(selected, None);
}

#[test]
fn upstream_nomount_version_is_rejected() {
    let mut kernel = MockKernel {
        version: Some("20"),
        applied: 0,
    };

    let err = select_provider(&mut kernel, SUPPORTED_VERSIONS, false).unwrap_err();

    assert!(matches!(err, Error::VfsUnsupportedVersion { .. }));
}

#[test]
fn foreign_nomount_is_refused_without_probing_k2() {
    let mut kernel = MockKernel {
        version: Some("hm1"),
        applied: 0,
    };

    let err = select_provider(&mut kernel, SUPPORTED_VERSIONS, true).unwrap_err();

    assert!(matches!(err, Error::VfsForeignNomount { .. }));
}

#[test]
fn exchange_rejects_request_that_is_not_one_page() {
    let mut kernel = KeyringKernel::new(KeyringChannel::Hybridmount).unwrap();

    let short = kernel.exchange(&[0_u8; 8]).unwrap_err();
    assert!(matches!(short, Error::VfsProtocol { .. }));

    let full = kernel.exchange(&[0_u8; crate::vfs::protocol::PAYLOAD_LEN]);
    assert!(matches!(full, Err(Error::Io(_))));
}
