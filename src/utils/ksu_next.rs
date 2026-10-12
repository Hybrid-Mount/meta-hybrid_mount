// SPDX-License-Identifier: GPL-3.0-only

//! KernelSU Next userspace identity, paired with a live KSU kernel query.
//! This identifies the running userspace environment, not the kernel's source
//! lineage: a Next daemon can also be paired with a compatible third-party kernel.

use crate::sys::process::{CaptureMode, CommandSpec, run_command};
use std::time::Duration;

const KSUD: &str = "/data/adb/ksud";

pub(crate) fn is_runtime() -> bool {
    detect_with(|args| {
        // Absolute path, separate arguments, successful exit and bounded output.
        // Both queries are read-only and work with old Next prctl kernels too.
        let spec = CommandSpec::new(KSUD)
            .args(args.iter().copied())
            .operation("identify KernelSU Next runtime")
            .timeout(Duration::from_secs(2))
            .capture(CaptureMode::Stdout);
        let outcome = run_command(&spec).ok()?;
        let stdout = outcome.stdout?;
        (stdout.omitted_bytes() == 0).then(|| stdout.render())
    })
}

fn detect_with(mut query: impl FnMut(&[&str]) -> Option<String>) -> bool {
    let Some(help) = query(&["--help"]) else {
        return false;
    };
    if help.trim().lines().next() != Some("KernelSU Next userspace cli") {
        return false;
    }
    let Some(version) = query(&["debug", "version"]) else {
        return false;
    };
    let Some(code) = version.trim().strip_prefix("Kernel Version: ") else {
        return false;
    };
    // No version-range heuristic: require only a positive live-kernel answer.
    !code.is_empty()
        && code.bytes().all(|byte| byte.is_ascii_digit())
        && code.parse::<u32>().is_ok_and(|value| value > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detect(help: Option<&str>, version: Option<&str>) -> bool {
        detect_with(|args| match args {
            ["--help"] => help.map(str::to_owned),
            ["debug", "version"] => version.map(str::to_owned),
            _ => panic!("unexpected or mutating ksud query: {args:?}"),
        })
    }

    #[test]
    fn next_brand_and_live_kernel_query_are_both_required() {
        assert!(detect(
            Some("KernelSU Next userspace cli\n\nUsage: ksud <COMMAND>\n"),
            Some("Kernel Version: 33442\n")
        ));
        // Old Next releases use the same header and query output, regardless of version range.
        assert!(detect(
            Some("KernelSU Next userspace cli\n\nUsage: ksud <COMMAND>\n"),
            Some("Kernel Version: 12345\n")
        ));
    }

    #[test]
    fn other_forks_or_incidental_mentions_do_not_get_exemption() {
        for help in [
            "KernelSU userspace cli\n",
            "SukiSU Ultra userspace cli\n",
            "KernelSU userspace cli\nSupports KernelSU Next userspace cli\n",
            "KernelSU Next userspace cli fork\n",
        ] {
            assert!(!detect(Some(help), Some("Kernel Version: 33442\n")));
        }
    }

    #[test]
    fn missing_failed_or_malformed_kernel_queries_do_not_exempt() {
        for version in [
            None,
            Some("Kernel Version: 0\n"),
            Some("Kernel Version: -1\n"),
            Some("Kernel Version: unknown\n"),
            Some("33442\n"),
            Some("Kernel Version: 33442\nERROR\n"),
        ] {
            assert!(!detect(Some("KernelSU Next userspace cli\n"), version));
        }
        assert!(!detect(None, Some("Kernel Version: 33442\n")));
    }

    #[test]
    fn an_unrecognized_daemon_never_runs_the_kernel_query() {
        let mut calls = 0;
        assert!(!detect_with(|args| {
            calls += 1;
            assert_eq!(args, ["--help"]);
            Some("KernelSU userspace cli\n".into())
        }));
        assert_eq!(calls, 1);
    }
}
