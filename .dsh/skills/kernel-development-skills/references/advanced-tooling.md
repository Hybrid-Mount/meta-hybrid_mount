# Advanced Kernel Tooling Reference

Load this reference only when the task needs specialized tooling beyond a
targeted build, checkpatch, local logs, KUnit, or kselftest.

## Tool Selection

| Need | Useful tools | Notes |
| --- | --- | --- |
| Patch series and review mail | `b4`, `git send-email`, `lei`, `patatt` | Prefer maintainer/subsystem norms; keep `Link:` and trailer metadata intact. |
| Fast cross-build coverage | TuxMake, TuxSuite, local cross toolchains | Match compiler, config, and architecture to the failing or reviewed target. |
| Boot and hardware CI | KernelCI, LAVA, labgrid | Preserve serial logs, boot artifacts, config, DTB, and exact image versions. |
| Static analysis | sparse, Smatch, Coccinelle, clang diagnostics | Use as evidence sources, not automatic truth. Confirm findings in code. |
| Fuzzing and coverage | syzkaller, KCOV, KASAN, KMSAN, KCSAN | Capture reproducer, config, console log, and suspected culprit before patching. |
| Runtime tracing | ftrace, trace-cmd, KernelShark, perf, eBPF/bpftrace, BCC, LTTng | Prefer existing tracepoints; keep filters narrow on embedded boards. |
| Quick boot/smoke test | virtme-ng (`vng`), QEMU | Boot the just-built kernel against the host rootfs for fast probe/regression checks before broader CI. |
| Crash forensics | pstore/ramoops, kdump, crash, drgn, GDB/vmlinux | Preserve unstripped `vmlinux`, modules, System.map, config, and vmcore/logs. |
| ABI and BTF inspection | pahole, bpftool, libabigail | Use when struct layout, BTF, UAPI, or module ABI is relevant. |

## Tool Availability

Probe tools before prescribing them:

```bash
command -v b4 sparse spatch smatch trace-cmd perf bpftrace crash drgn pahole
```

`command -v` with multiple names prints only the tools that exist and exits
non-zero if any is missing, so check the output lines, not the exit code.

If a tool is unavailable, do not install or vendor it unless that is explicitly
in scope. Pick the nearest repo-native or already-installed check.

## Patch Workflow Tools

- Use `b4` for mailed series retrieval and preparation when working from lore.
- Use `scripts/get_maintainer.pl` plus subsystem docs before choosing reviewers.
- Use `patatt` only when the project/signing workflow expects patch attestation.
- Keep cover-letter change logs below the `---` separator so they do not become commit text.

## Static Analysis Discipline

- Run checkpatch for style signals, sparse for type/address-space issues, Smatch
  for path-sensitive warnings when available, and Coccinelle for semantic tree
  changes.
- Keep mechanical Coccinelle conversions separate from behavior changes.
- For false positives, document why in the review notes or commit message only
  when the warning is likely to reappear in review.

## Fuzzing And Sanitizer Intake

For syzkaller or sanitizer reports:

1. Save report URL, dashboard metadata, reproducer, config, compiler, commit, and console log.
2. Identify whether the crash is memory safety, locking, warning, leak, refcount, or API misuse.
3. Minimize the reproducer only after preserving the original artifacts.
4. Validate the fix with the original reproducer and one targeted negative check.
5. Add a regression test when the bug is reachable through stable behavior.

## Tracing Pipeline

- Start with logs and existing tracepoints; add new tracepoints only when they are useful beyond one debug session.
- Prefer `trace-cmd record/report` when the trace must be shared or viewed in KernelShark.
- Use `perf` for sampling and PMU questions; use ftrace for call graph and event flow.
- Use BPF/bpftrace for temporary observability only when kernel config, BTF, and permissions support it.
- On small boards, cap buffer sizes and filter aggressively to avoid perturbing timing.
