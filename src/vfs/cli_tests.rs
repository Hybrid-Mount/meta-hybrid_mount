// SPDX-License-Identifier: GPL-3.0-only

use super::*;

fn command(args: &[&str]) -> Result<Command> {
    parse(
        &args.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>(),
        Path::new("/work"),
    )
}

#[test]
fn all_legacy_aliases_match_standard_commands() {
    for (aliases, canonical, tail) in [
        (&["add", "a"][..], &["rule", "add"][..], &["/v", "/r"][..]),
        (&["del", "d"][..], &["rule", "del"][..], &["/v"][..]),
        (
            &["whiteout", "w"][..],
            &["rule", "add", "--whiteout"][..],
            &["/v"][..],
        ),
        (&["block", "b"][..], &["uid", "add"][..], &["10001"][..]),
        (&["unblock", "u"][..], &["uid", "del"][..], &["10001"][..]),
        (&["list", "l"][..], &["rule", "list"][..], &[][..]),
        (&["version", "v", "-v"][..], &["version"][..], &[][..]),
    ] {
        for alias in aliases {
            assert_eq!(
                command(&[&[*alias], tail].concat()).unwrap(),
                command(&[canonical, tail].concat()).unwrap(),
                "{alias}"
            );
        }
    }
    assert_eq!(
        command(&["l", "uid", "json"]).unwrap(),
        command(&["uid", "list", "--json"]).unwrap()
    );
    assert_eq!(
        command(&["list", "uid"]).unwrap(),
        command(&["uid", "list"]).unwrap()
    );
}

#[test]
fn batch_add_normalizes_paths_and_carries_requested_uid() {
    let parsed = command(&[
        "rule",
        "add",
        "--uid",
        "10234",
        "a/../v",
        "./real",
        "/other/",
        "/source//x",
        "--json",
    ])
    .unwrap();
    assert_eq!(
        parsed,
        Command {
            json: true,
            operation: Operation::Mutate(Mutation::Add {
                uid: 10234,
                rules: vec![
                    EncodedRule {
                        flags: 0,
                        virtual_path: b"/work/v".to_vec(),
                        real_path: b"/work/real".to_vec()
                    },
                    EncodedRule {
                        flags: 0,
                        virtual_path: b"/other".to_vec(),
                        real_path: b"/source/x".to_vec()
                    },
                ]
            })
        }
    );
}

#[test]
fn whiteout_and_opaque_are_batched_without_real_paths() {
    for (flag, expected) in [
        ("--whiteout", protocol::FLAG_WHITEOUT),
        ("--opaque", protocol::FLAG_OPAQUE),
    ] {
        let parsed = command(&["rule", "add", flag, "/a", "/b", "--uid=44"]).unwrap();
        let Operation::Mutate(Mutation::Add { rules, uid }) = parsed.operation else {
            panic!("wrong operation")
        };
        assert_eq!(uid, 44);
        assert_eq!(rules.len(), 2);
        assert!(
            rules
                .iter()
                .all(|r| r.flags == expected && r.real_path.is_empty())
        );
    }
}

#[test]
fn clear_requires_explicit_scope_and_confirmation() {
    for bad in [
        vec!["clear"],
        vec!["clear", "--yes"],
        vec!["clear", "typo", "--yes"],
        vec!["clear", "all"],
        vec!["rule", "clear"],
        vec!["uid", "clear"],
        vec!["clear", "all", "--yes", "--uid", "10"],
    ] {
        assert_eq!(command(&bad).unwrap_err().exit_code(), 2, "{bad:?}");
    }
    for (args, scope) in [
        (vec!["clear", "rules", "--yes"], ClearScope::Rules),
        (vec!["rule", "clear", "--yes"], ClearScope::Rules),
        (vec!["clear", "uid", "--yes"], ClearScope::Uids),
        (vec!["uid", "clear", "--yes"], ClearScope::Uids),
        (vec!["clear", "all", "--yes"], ClearScope::All),
    ] {
        assert_eq!(
            command(&args).unwrap().operation,
            Operation::Mutate(Mutation::Clear(scope))
        );
    }
}

#[test]
fn invalid_arguments_fail_before_contacting_provider() {
    for args in [
        vec!["rule"],
        vec!["rule", "missing"],
        vec!["rule", "add"],
        vec!["rule", "add", "/a", "/b", "/odd"],
        vec!["rule", "del"],
        vec!["uid", "add"],
        vec!["uid", "add", "42", "invalid"],
        vec!["uid", "add", "-1"],
        vec!["uid", "add", "+1"],
        vec!["uid", "add", "4294967296"],
        vec!["uid", "add", ""],
        vec!["rule", "add", "--whiteout", "--opaque", "/a"],
        vec!["rule", "add", "/a", "/b", "--uid"],
        vec!["rule", "add", "/a", "/b", "--uid", "1", "--uid=2"],
        vec!["rule", "list", "--uid", "1"],
        vec!["rule", "list", "extra"],
        vec!["rule", "del", "/a", "--whiteout"],
        vec!["version", "junk"],
        vec!["rule", "add", "", "/b"],
        vec!["rule", "add", "/a\0x", "/b"],
        vec!["uid", "del", "10", "--save"],
        vec!["unload"],
    ] {
        let owned: Vec<_> = args.iter().map(|arg| (*arg).to_owned()).collect();
        assert_eq!(handle(&owned).unwrap_err().exit_code(), 2, "{args:?}");
    }
}

#[test]
fn oversized_later_record_rejects_entire_command() {
    let path = format!("/{}", "x".repeat(4060));
    assert_eq!(
        command(&["rule", "add", "/v", "/r", &path, "/source"])
            .unwrap_err()
            .exit_code(),
        2
    );
    assert_eq!(
        command(&["rule", "del", &format!("/{}", "x".repeat(4062))])
            .unwrap_err()
            .exit_code(),
        2
    );
    assert_eq!(
        normalize_path(&format!("/{}", "x".repeat(4095)), Path::new("/work"))
            .unwrap_err()
            .exit_code(),
        2
    );
}

#[test]
fn option_terminator_and_literal_json_remain_paths() {
    let parsed = command(&["a", "--", "--json", "json"]).unwrap();
    assert!(!parsed.json);
    let Operation::Mutate(Mutation::Add { rules, .. }) = parsed.operation else {
        panic!("wrong operation")
    };
    assert_eq!(rules[0].virtual_path, b"/work/--json");
    assert_eq!(rules[0].real_path, b"/work/json");
    assert!(!command(&["a", "json", "/source"]).unwrap().json);
    assert_eq!(command(&["l", "--", "json"]).unwrap_err().exit_code(), 2);
}

#[test]
fn uid_boundary_values_and_duplicates_are_handled() {
    assert_eq!(
        command(&["uid", "add", "4294967295", "0", "01", "1"])
            .unwrap()
            .operation,
        Operation::Mutate(Mutation::AddUids(vec![0, 1, u32::MAX]))
    );
    assert_eq!(
        command(&["d", "/a", "/b", "--uid=0"]).unwrap().operation,
        Operation::Mutate(Mutation::Delete {
            paths: vec![b"/a".to_vec(), b"/b".to_vec()],
            uid: 0
        })
    );
}

#[test]
fn help_and_load_are_distinct_from_plain_pipeline_entrypoint() {
    assert_eq!(command(&[]).unwrap().operation, Operation::Help);
    for args in [
        vec!["help"],
        vec!["--help"],
        vec!["rule", "--help"],
        vec!["rule", "add", "--help"],
        vec!["clear", "--help"],
    ] {
        assert_eq!(command(&args).unwrap().operation, Operation::Help);
    }
    assert_eq!(
        command(&["load", "--json"]).unwrap(),
        Command {
            operation: Operation::Load,
            json: true
        }
    );
    assert_eq!(Error::msg("runtime failure").exit_code(), 1);
}

#[test]
fn normalized_virtual_path_does_not_need_to_exist() {
    assert_eq!(
        normalize_path("/missing/../new//path/", Path::new("/work")).unwrap(),
        b"/new/path"
    );
    assert_eq!(
        normalize_path("../../x", Path::new("/work")).unwrap(),
        b"/x"
    );
    assert!(normalize_path("relative", Path::new("")).is_err());
    assert_eq!(
        normalize_path("/absolute", Path::new("")).unwrap(),
        b"/absolute"
    );
}

#[test]
fn rule_json_escapes_paths_and_exposes_hm_flags() {
    let row = ListedRule {
        flags: protocol::FLAG_OPAQUE | protocol::FLAG_VIRTUAL_DIR | 1,
        uid: 12,
        virtual_path: "/a\"\n\\b".into(),
        real_path: String::new(),
    };
    let output = rules_json(std::slice::from_ref(&row)).unwrap();
    let json: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(json[0]["virtual_path"], row.virtual_path);
    assert_eq!(json[0]["uid"], 12);
    assert_eq!(json[0]["opaque"], true);
    assert_eq!(json[0]["virtual_dir"], true);
    assert_eq!(json[0]["whiteout"], false);
    assert!(rules_text(&[row]).contains("opaque directory"));
    assert_eq!(rules_json(&[]).unwrap(), "[]");
}

#[test]
fn doctor_json_remains_compatible_and_text_contains_errors() {
    let mut report = doctor::summarize(
        None,
        doctor::ModulePresence::NotPresent,
        Ok((Vec::new(), Vec::new())),
    );
    report.probe_error = Some("probe denied".into());
    let text = doctor_text(&report);
    assert!(text.contains("probe denied"));
    assert!(text.contains("NotPresent"));
    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(json["key_type"], "hybridmount");
    assert_eq!(json["responds"], false);
}
