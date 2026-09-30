# Kernel CI And Regression Reference

Use this when a kernel task involves hosted CI, lab boot failures, flaky tests,
syzkaller reports, stable trees, or user-visible regressions.

## First Triage

1. Capture exact artifact identity: commit, branch, config, architecture, compiler, DTB, rootfs, firmware, and test name.
2. Classify the failure as build, boot, probe, runtime test, performance, flake, infra, or unrelated baseline.
3. Compare the first failing commit and changed files against the failing config and subsystem.
4. Reproduce locally with the smallest equivalent target when practical.
5. Preserve the first error, warning, oops, or failed assertion. Later failures can be fallout.

## Provider Notes

| Surface | Evidence to keep |
| --- | --- |
| KernelCI | job URL, tree/branch, config, DTB, lab, board, serial log, boot result, test suite |
| LAVA | job definition, device type, pipeline actions, boot log, test shell output, infrastructure errors |
| TuxSuite/TuxMake/TuxRun | target arch, compiler, config, build log, artifact URL, failing command (TuxMake and TuxRun now live under the KernelCI project; TuxSuite remains the hosted service) |
| syzkaller | dashboard/report URL, reproducer, `.config`, compiler, commit, console log, C reproducer if available |
| GitHub Actions | workflow run URL, job log, artifact list, changed paths, cache/toolchain versions |

Treat hosted provider names as evidence locations, not proof of root cause.

## Regression Handling

Kernel regression policy is based on user-visible breakage. A cleaner design that
breaks an existing workflow is still a regression until handled.

- Reproduce or narrowly characterize the breakage.
- Identify the culprit via bisect, range analysis, or subsystem history.
- CC `regressions@lists.linux.dev` when reporting/submitting relevant upstream fixes.
- Use regzbot commands for mailed reports when tracking helps. Use the plain
  form in your own report; the caret form only when replying to someone else's
  report:

```text
#regzbot introduced: <commit-or-version-range>    (in your own report)
#regzbot ^introduced: <commit-or-version-range>   (replying to a report)
#regzbot title: <short description>
#regzbot fix: <patch subject or commit>
#regzbot monitor: <lore thread URL>
```

- Include a `Closes:` (or `Link:`) tag in the fix pointing at the regression
  report so regzbot can connect the fix and resolve the tracked entry.
- A regression fix should be small and easy to backport; if the ideal cleanup is
  larger, consider a minimal fix first and cleanup later.
- Upstream expects regression fixes to be mainlined quickly: roughly within two
  weeks for ordinary regressions and days for severe or widely hit ones.

## Stable And LTS

For stable candidates:

- Confirm the fix is for a real bug, not a feature or refactor.
- The patch (or an equivalent fix) must be in mainline before a stable tree
  applies it; tagging `Cc: stable` at upstream submission time is fine.
- Include `Fixes:` when the culprit is known.
- Add `Cc: stable@vger.kernel.org` only when stable rules are met.
- Document backport conflicts and any behavior delta from upstream.

For vendor/LTS kernels:

- Compare upstream, LTS, vendor branches, DTS, firmware, and userspace packages.
- Preserve upstream metadata when cherry-picking.
- Validate against the vendor integration path, not only upstream build success.

## Flake Discipline

Do not mark a failure flaky without evidence:

- baseline runs showing the same failure without the patch,
- provider incident or infrastructure error,
- test history showing intermittent behavior across unrelated commits, or
- a successful rerun with unchanged artifacts plus a plausible infra signature.

When the change touches timing, PM, locking, media, storage, networking, or
thermal behavior, assume a "flake" may be a real race until disproven.

## Reporting Checklist

- Failing URL/artifact and exact command are recorded.
- First failing log line and first bad commit/range are identified.
- Local reproduction was attempted or explained.
- Related maintainers/subsystem docs are checked.
- Stable/regression tags are considered.
- Validation evidence includes the original failure mode after the fix.
