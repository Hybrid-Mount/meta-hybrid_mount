// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use crate::errors::Error;

fn rule(virtual_path: &str) -> ListedRule {
    ListedRule {
        flags: 0,
        uid: 0,
        virtual_path: virtual_path.to_owned(),
        real_path: "/data/adb/modules/m/x".to_owned(),
    }
}

#[test]
fn a_loaded_module_is_listed_in_proc_modules() {
    assert_eq!(classify_presence(true, true), ModulePresence::Loadable);
}

/// The discriminator: `/proc/modules` only lists loadable modules, so an answering key
/// type with a `/sys/module` entry but no `/proc/modules` entry is compiled in.
#[test]
fn a_key_type_with_only_a_sys_module_entry_is_built_in() {
    assert_eq!(classify_presence(false, true), ModulePresence::BuiltIn);
}

#[test]
fn neither_table_entry_means_the_module_is_absent() {
    assert_eq!(classify_presence(false, false), ModulePresence::NotPresent);
}

#[test]
fn proc_modules_matching_reads_the_first_field_only() {
    let text = "ext4 100 0 - Live 0x0\nhybridmount 16384 0 - Live 0x0 (O)\n";
    assert!(listed_in_proc_modules(text));
}

#[test]
fn proc_modules_matching_does_not_match_a_longer_name() {
    let text = "hybridmount_extra 16384 0 - Live 0x0 (O)\n";
    assert!(!listed_in_proc_modules(text));
}

#[test]
fn proc_modules_matching_ignores_an_unrelated_table() {
    assert!(!listed_in_proc_modules("ext4 100 0 - Live 0x0\n"));
    assert!(!listed_in_proc_modules(""));
}

/// The question the user actually asks: with hybridmount compiled in, is another
/// provider required? It is not, and an `insmod` could only fail on the name.
#[test]
fn a_built_in_kernel_needs_no_provider_install() {
    assert!(!provider_install_required(ModulePresence::BuiltIn, true));
    assert!(!provider_install_required(ModulePresence::BuiltIn, false));
}

#[test]
fn an_already_registered_key_type_needs_no_provider_install() {
    assert!(!provider_install_required(ModulePresence::Loadable, true));
}

#[test]
fn a_missing_provider_requires_an_independent_install() {
    assert!(provider_install_required(ModulePresence::NotPresent, false));
}

#[test]
fn a_built_in_kernel_is_diagnosed_as_needing_no_module() {
    let text = diagnose(ModulePresence::BuiltIn, Some("hm1"), true);
    assert!(text.contains("built into"), "diagnosis was {text}");
    assert!(text.contains("no module"), "diagnosis was {text}");
}

#[test]
fn a_loaded_module_is_diagnosed_as_registered() {
    let text = diagnose(ModulePresence::Loadable, Some("hm1"), true);
    assert!(text.contains("loaded module"), "diagnosis was {text}");
}

#[test]
fn a_missing_key_type_points_at_an_independent_provider() {
    let text = diagnose(ModulePresence::NotPresent, None, false);
    assert!(
        text.contains("separately installed"),
        "diagnosis was {text}"
    );
}

/// The magic mismatch signature: the key type is there but did not answer.
#[test]
fn a_silent_key_type_points_at_the_magic() {
    let built_in = diagnose(ModulePresence::BuiltIn, None, false);
    assert!(built_in.contains("magic"), "diagnosis was {built_in}");

    let loaded = diagnose(ModulePresence::Loadable, None, false);
    assert!(loaded.contains("magic"), "diagnosis was {loaded}");
}

#[test]
fn an_unsupported_version_is_diagnosed_as_skipped() {
    let text = diagnose(ModulePresence::Loadable, Some("20"), false);
    assert!(text.contains("not support"), "diagnosis was {text}");
}

#[test]
fn summarize_carries_both_tables_when_listing_succeeded() {
    let report = summarize(
        Some("hm1".to_owned()),
        ModulePresence::BuiltIn,
        Ok((vec![rule("/system/etc/hosts")], vec![1000, 10123])),
    );

    assert_eq!(report.provider, "hm");
    assert_eq!(report.key_type, "hybridmount");
    assert_eq!(report.version, Some("hm1".to_owned()));
    assert!(report.responds);
    assert!(!report.provider_install_required);
    assert_eq!(report.rules.len(), 1);
    assert_eq!(report.uids, vec![1000, 10123]);
    assert!(report.list_error.is_none());
}

/// A listing failure must not hide the binding result, which is the more useful half.
#[test]
fn summarize_reports_a_listing_failure_without_losing_the_probe() {
    let report = summarize(
        Some("hm1".to_owned()),
        ModulePresence::BuiltIn,
        Err(Error::VfsProtocol {
            detail: "truncated".to_owned(),
        }),
    );

    assert_eq!(report.version, Some("hm1".to_owned()));
    assert!(report.rules.is_empty());
    assert!(report.uids.is_empty());
    let err = report.list_error.expect("a listing failure is surfaced");
    assert!(err.contains("truncated"), "error was {err}");
}

#[test]
fn summarize_without_a_probe_reports_the_module_as_needed() {
    let report = summarize(
        None,
        ModulePresence::NotPresent,
        Ok((Vec::new(), Vec::new())),
    );

    assert!(!report.responds);
    assert!(report.provider_install_required);
    assert_eq!(report.version, None);
}

#[test]
fn the_report_serializes_to_snake_case() {
    let report = summarize(None, ModulePresence::BuiltIn, Ok((Vec::new(), Vec::new())));
    let json = serde_json::to_string(&report).unwrap();

    assert!(
        json.contains("\"presence\":\"built_in\""),
        "json was {json}"
    );
    assert!(
        json.contains("\"key_type\":\"hybridmount\""),
        "json was {json}"
    );
}

#[test]
fn existing_providers_block_loading_even_when_the_probe_is_silent() {
    for presence in [ModulePresence::BuiltIn, ModulePresence::Loadable] {
        let err = ensure_provider_absent(presence).unwrap_err();
        assert!(err.contains("refusing duplicate insmod/unload"));
    }
    assert!(ensure_provider_absent(ModulePresence::NotPresent).is_ok());
}
