// SPDX-License-Identifier: GPL-3.0-only

use std::cell::Cell;
use std::rc::Rc;

use super::*;
use crate::errors::Error;
use crate::vfs::protocol::EncodedRule;

/// The version lives behind a handle so a load closure can make the provider start
/// answering without holding a second mutable borrow of the kernel itself.
struct MockKernel {
    version: Rc<Cell<Option<&'static str>>>,
    applied: usize,
}

impl MockKernel {
    fn answering(version: &'static str) -> Self {
        Self {
            version: Rc::new(Cell::new(Some(version))),
            applied: 0,
        }
    }

    fn absent() -> Self {
        Self {
            version: Rc::new(Cell::new(None)),
            applied: 0,
        }
    }

    fn version_handle(&self) -> Rc<Cell<Option<&'static str>>> {
        Rc::clone(&self.version)
    }
}

impl VfsKernel for MockKernel {
    fn version(&mut self) -> Result<String> {
        self.version
            .get()
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

    fn list_rules(&mut self) -> Result<Vec<crate::vfs::protocol::ListedRule>> {
        Ok(Vec::new())
    }
}

#[test]
fn existing_k2_is_adopted() {
    let mut kernel = MockKernel::answering("hm1");

    let selected = select_provider(&mut kernel, SUPPORTED_VERSIONS, false, || Ok(())).unwrap();

    assert_eq!(selected, Some(VfsProvider::Hm));
}

/// The bundled module is the only reason a device without a built-in provider gets the backend,
/// so a silent probe must trigger exactly one load attempt.
#[test]
fn a_silent_probe_loads_the_bundled_module_once() {
    let mut kernel = MockKernel::absent();
    let version = kernel.version_handle();
    let mut loads = 0;

    let selected = select_provider(&mut kernel, SUPPORTED_VERSIONS, false, || {
        loads += 1;
        version.set(Some("hm1"));
        Ok(())
    })
    .unwrap();

    assert_eq!(selected, Some(VfsProvider::Hm));
    assert_eq!(loads, 1);
}

/// A provider that already answers must not be disturbed by an insmod.
#[test]
fn an_answering_provider_is_never_reloaded() {
    let mut kernel = MockKernel::answering("hm1");

    select_provider(&mut kernel, SUPPORTED_VERSIONS, false, || {
        panic!("an answering key type must not trigger a load")
    })
    .unwrap();
}

/// An unsupported version is a deliberate refusal, not a reason to load another module.
#[test]
fn an_unsupported_version_does_not_trigger_a_load() {
    let mut kernel = MockKernel::answering("20");

    let err = select_provider(&mut kernel, SUPPORTED_VERSIONS, false, || {
        panic!("an unsupported provider must not trigger a load")
    })
    .unwrap_err();

    assert!(matches!(err, Error::VfsUnsupportedVersion { .. }));
}

#[test]
fn missing_provider_stays_unavailable_when_the_load_does_not_help() {
    let mut kernel = MockKernel::absent();

    let selected = select_provider(&mut kernel, SUPPORTED_VERSIONS, false, || Ok(())).unwrap();

    assert_eq!(selected, None);
}

#[test]
fn upstream_nomount_version_is_rejected() {
    let mut kernel = MockKernel::answering("20");

    let err = select_provider(&mut kernel, SUPPORTED_VERSIONS, false, || Ok(())).unwrap_err();

    assert!(matches!(err, Error::VfsUnsupportedVersion { .. }));
}

#[test]
fn foreign_nomount_is_refused_without_probing_k2() {
    let mut kernel = MockKernel::answering("hm1");

    let err = select_provider(&mut kernel, SUPPORTED_VERSIONS, true, || {
        panic!("a foreign provider must not trigger a load")
    })
    .unwrap_err();

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

#[test]
fn rejected_magic_is_reported_as_an_unprocessed_payload() {
    let response = protocol::build_payload(protocol::NmCommand::GetVersion, 0, &[]).unwrap();
    let err = parse_version_response(&response).unwrap_err().to_string();
    assert!(err.contains("GET_VERSION payload was not processed"));
    assert!(err.contains("0x4859425249444d4f"));
    assert!(err.contains("rebuild the kernel"));
}

#[test]
fn processed_version_response_is_still_accepted() {
    let mut response = protocol::build_payload(protocol::NmCommand::GetVersion, 0, b"hm1").unwrap();
    response[16..20].copy_from_slice(&0_i32.to_le_bytes());
    assert_eq!(parse_version_response(&response).unwrap(), "hm1");
}

#[test]
fn truncated_version_response_remains_an_error() {
    assert!(parse_version_response(&[]).is_err());
}
