# Kernel Debugging And Testing Reference

## Table Of Contents

- Source map
- Debugging workflow
- Runtime diagnostics
- Tracing
- Sanitizers and hardening tools
- KUnit and kselftest
- Static analysis
- CI failure triage
- Hardware validation

## Source Map

Local docs to check first:

- `Documentation/dev-tools/testing-overview.rst`
- `Documentation/dev-tools/kunit/`
- `Documentation/dev-tools/kselftest.rst`
- `Documentation/dev-tools/ktap.rst`
- `Documentation/dev-tools/checkpatch.rst`
- `Documentation/dev-tools/sparse.rst`
- `Documentation/dev-tools/coccinelle.rst`
- `Documentation/dev-tools/kasan.rst`
- `Documentation/dev-tools/kmsan.rst`
- `Documentation/dev-tools/kcsan.rst`
- `Documentation/dev-tools/kfence.rst`
- `Documentation/admin-guide/dynamic-debug-howto.rst`
- `Documentation/trace/ftrace.rst`
- `Documentation/fault-injection/`
- `Documentation/dev-tools/kgdb.rst`
- `Documentation/dev-tools/gdb-kernel-debugging.rst`
- `Documentation/admin-guide/ramoops.rst`
- `Documentation/admin-guide/kdump/`

Current upstream references:

- https://docs.kernel.org/dev-tools/testing-overview.html
- https://docs.kernel.org/dev-tools/kunit/
- https://docs.kernel.org/dev-tools/kselftest.html
- https://docs.kernel.org/dev-tools/ktap.html
- https://docs.kernel.org/admin-guide/dynamic-debug-howto.html
- https://docs.kernel.org/trace/ftrace.html
- https://docs.kernel.org/fault-injection/fault-injection.html
- https://docs.kernel.org/dev-tools/kasan.html
- https://docs.kernel.org/dev-tools/kcsan.html
- https://docs.kernel.org/dev-tools/kfence.html
- https://docs.kernel.org/dev-tools/kgdb.html
- https://docs.kernel.org/admin-guide/ramoops.html
- https://github.com/google/syzkaller/tree/master/docs

For deeper tooling choices, load `advanced-tooling.md`. For hosted CI, LAVA,
KernelCI, TuxSuite, syzkaller dashboard, or regression-report handling, load
`ci-regression.md`.

If a repository provides a kernel build wrapper, use it for every build or test
target. In this repository that wrapper is `cargo xtask` plus the DDK-based
`kernel-module.yml`; do not translate the `make` examples below into a direct
kernel build.

## Debugging Workflow

1. Capture the exact symptom: kernel version, config, board, boot args, module state, workload, and complete logs.
2. Classify the failure: build, boot, probe, runtime, race, memory safety, lockup, performance, ABI, or power/PM.
3. Reduce scope: reproduce with the smallest config, device, module, or workload that still fails.
4. Add observability before changing behavior. Prefer existing tracepoints, dynamic debug, debugfs/sysfs, and subsystem logs.
5. Form one hypothesis at a time and collect evidence that can disprove it.
6. After the fix, rerun the original reproducer and one adjacent regression check.

## Runtime Diagnostics

Use `dmesg -w`, `journalctl -k -b`, or serial logs first. Preserve the first warning/oops; later failures can be fallout.

Dynamic debug:

```bash
cat /proc/dynamic_debug/control
echo 'file drivers/foo/* +p' > /proc/dynamic_debug/control
echo 'module foo +p' > /proc/dynamic_debug/control
echo 'func foo_probe +p' > /proc/dynamic_debug/control
```

Notes:

- Needs `CONFIG_DYNAMIC_DEBUG` or appropriate dynamic debug core support.
- `dev_dbg()` and `pr_debug()` output may still need loglevel or console settings.
- Prefer dynamic debug over permanent noisy prints.

Crash persistence:

- Use pstore/ramoops when failures reboot the board before logs can be copied.
- For panic analysis on larger systems, configure kdump/crash capture if available.
- Record taint flags; tainted kernels affect upstream triage.

## Tracing

Ftrace/tracefs is often the fastest path for kernel behavior and latency questions:

```bash
mount -t tracefs nodev /sys/kernel/tracing
cd /sys/kernel/tracing
cat available_tracers
echo function_graph > current_tracer
echo 1 > tracing_on
cat trace
echo 0 > tracing_on
```

Event tracing:

```bash
echo 1 > events/<subsystem>/<event>/enable
cat trace_pipe
```

Guidance:

- Prefer existing tracepoints over adding ad hoc printk.
- Use filters and `set_ftrace_filter` to limit overhead and log volume.
- Capture before/after traces for behavior changes.
- Use `perf` for CPU sampling, lock/contention, and hardware PMU questions when available.
- Use BPF/bpftrace only when the target kernel and environment support it; do not make it a mandatory validation path for embedded boards.

## Sanitizers And Hardening Tools

Pick tools based on bug class and runtime budget:

| Tool | Use for |
| --- | --- |
| KASAN | out-of-bounds and use-after-free memory bugs; high overhead |
| KMSAN | uninitialized memory reads; clang-only and x86_64-only, so typically specialist builds |
| KCSAN | data races; useful for concurrency changes |
| UBSAN | undefined behavior checks |
| KFENCE | lower-overhead memory bug detection, useful for longer runs |
| lockdep | lock ordering, IRQ context, deadlock risk |
| kmemleak | leaked allocations |
| fault injection | error-path coverage for allocation, block, usercopy, futex, function failures |

For locking changes, run with lockdep if feasible before review. For allocation/error-path work, use fault injection or explicit failure hooks when available.

## KUnit And Kselftest

KUnit:

- Best for internal logic, helpers, parsers, data-structure behavior, and driver code that can be isolated.
- Results are KTAP formatted.
- Use `tools/testing/kunit/kunit.py` when the repo/toolchain supports it.

Typical upstream usage:

```bash
./tools/testing/kunit/kunit.py run
./tools/testing/kunit/kunit.py run '<suite-or-glob>'
```

Kselftest:

- Best for user-visible kernel behavior, syscalls, networking, BPF, cgroups, drivers with userspace interfaces, and regression tests.
- Tests live under `tools/testing/selftests/`.
- Many stable trees run newer kselftests against older kernels, so tests should handle unsupported features carefully.

Typical upstream usage:

```bash
make -C tools/testing/selftests TARGETS=<target> run_tests
make TARGETS="<target> <target>" kselftest          # from the kernel top level
./run_kselftest.sh -c <collection> -t <collection>:<test>   # installed runner
```

In repos with wrappers, replace direct `make` with the repo-prescribed wrapper.

## Static Analysis

Use static tools before broader runtime work:

```bash
scripts/checkpatch.pl --strict <patch-file>
make C=1 <target>          # upstream-style sparse invocation
spatch --sp-file <rule.cocci> <files>
```

Guidance:

- Checkpatch is not an authority; fix real issues and explain intentional exceptions.
- Sparse is valuable for address-space annotations, endianness, locking annotations, and API misuse.
- Smatch is useful when available for deeper path-sensitive checks; treat it as advisory and confirm real bugs in code.
- Coccinelle is useful for tree-wide mechanical API transitions; keep semantic patches separate from behavior changes.

## CI Failure Triage

1. Identify whether the failure is build, test, boot, runtime, flaky infrastructure, or unrelated baseline.
2. Compare first failing commit and failing config to the changed files.
3. Reproduce the smallest failing target locally or with the same config.
4. For build failures, inspect generated config, include paths, compiler, warnings-as-errors, and dependency ordering.
5. For boot/runtime failures, capture full serial log and probe order.
6. Do not mark CI as unrelated unless there is concrete baseline evidence.

When CI uses KernelCI, TuxSuite/TuxMake, LAVA, or syzkaller, use
`ci-regression.md` for provider-specific triage and evidence capture.

## Hardware Validation

For board or driver changes, include evidence that matches the hardware path:

- Boot log lines for probe, power rails, clocks, resets, IRQs, and firmware.
- Relevant `/sys`, `/proc`, `debugfs`, `tracefs`, or device node state.
- Bus-level evidence when useful: `i2cdetect`, `i2cget`, `lspci`, `lsusb`, `ethtool`, `media-ctl`, `v4l2-ctl`, depending on subsystem.
- Before/after power, thermal, suspend/resume, hotplug, or error-path checks when the change touches those behaviors.
