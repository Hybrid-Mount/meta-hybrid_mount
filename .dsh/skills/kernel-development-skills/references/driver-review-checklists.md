# Driver Review And Maintenance Checklists

Load this when reviewing, maintaining, debugging, backporting, or preparing a
Linux kernel driver change. Pair it with `driver-api-cookbook.md` for API
details.

## Pre-Edit

- Read nearest repo guidance, current `git status --short`, `MAINTAINERS`,
  local `Documentation/`, subsystem docs, Kconfig, Makefile, binding, and two
  nearby drivers with similar hardware.
- Identify public surfaces before editing: UAPI headers, sysfs, ioctl, netlink,
  tracepoints, DT bindings, module parameters, firmware names, and userspace
  package dependencies.
- Determine whether the change is a fix, feature, refactor, backport, vendor
  policy change, or board integration change. Split those intents unless the
  repository explicitly wants one local commit.
- For board work, verify schematic/datasheet -> active DTS -> binding -> driver
  -> resolved config -> runtime evidence. Do not treat a fragment or old doc as
  resolved truth.
- For suspected regressions, preserve the original failure log/reproducer before
  changing code.

## Driver Code Review

| Area | Questions |
| --- | --- |
| Probe ordering | Are resources acquired before hardware enable, state initialized before callbacks, and subsystem registration last? |
| Error paths | Does every failure return the original errno, log enough context, and unwind enabled hardware? |
| Deferred probe | Are supplier failures returned, logged with `dev_err_probe()`, and visible in `devices_deferred`? |
| Lifetime | Can any IRQ, timer, work item, callback, or child device access freed state during remove or failed probe? Does the `remove()` signature match the target tree (`void` on current platform/i2c/spi, `int` on older LTS platform drivers), with no error "returned" from remove paths? |
| Devres | Is `devm_*` used for probe-bound resources, and is explicit cleanup kept where ordering matters? |
| Locking/context | Are sleepable operations out of hard IRQ/spinlock/atomic context? Are lock ordering and callback reentry considered? |
| PM | Are runtime PM gets/puts balanced, suspend paths serialized, and wakeup behavior explicit? |
| MMIO/bus IO | Are accessors, barriers/readbacks, endianness, and regmap choices appropriate for the bus? |
| DMA | Is the DMA mask set before DMA, and are map/unmap/sync/error paths balanced? |
| GPIO/pinctrl | Are descriptor APIs and logical levels used, with `_cansleep` where required? |
| Power resources | Are regulators, clocks, resets, and power domains enabled/disabled in a defensible order? |
| ABI | Are user-visible changes documented and backward compatible? |
| Kconfig | Are dependencies sufficient for built-in, module, COMPILE_TEST, and disabled-subsystem builds? |

## Build And Static Checks

- Run `git diff --check`.
- Use `scripts/checkpatch.pl --strict <patch-file>` as a signal, not as final
  authority. Explain intentional exceptions that reviewers are likely to see.
- Build the focused object/subsystem target through the repo-approved wrapper.
  When supported, test both built-in and module forms.
- Run sparse (`make C=1 <target>` or wrapper equivalent) for address-space,
  endianness, locking annotation, and API misuse checks when practical.
- Use Smatch, Coccinelle, clang diagnostics, or sanitizer builds only when they
  fit the change risk and are available; do not invent new tool dependencies as
  a prerequisite for a small patch.
- Run `check_kernel_api_patterns.py` only as an advisory pre-review scan. Kernel
  docs, compiler output, static analysis, and human review remain authoritative.

## Runtime And Hardware Evidence

- Capture full boot or module-load logs, not only the final success line.
- Preserve the first warning/oops/error; later failures can be fallout.
- For probe failures, capture `/sys/kernel/debug/devices_deferred`, relevant
  supplier summaries, and dynamic debug output when useful.
- For GPIO/pinctrl changes, capture `debugfs` GPIO/pinctrl state and verify
  active-low/open-drain semantics against hardware.
- For regulator/clock/reset/power-domain changes, capture framework summaries
  when available and prove the device still works after unused suppliers are
  gated.
- For IRQ/work/timer changes, test remove/unbind or module unload when possible.
- For PM changes, test runtime suspend/resume, system suspend/resume when in
  scope, and wakeup/error paths touched by the patch.
- For DMA changes, test data integrity under load and the original cache/coherency
  failure mode when one exists.

## Maintenance And Backports

- Prefer upstream commits over hand-written vendor fixes. Preserve original
  author, commit message, `Fixes:`, `Link:`, and stable metadata when possible.
- Document conflict resolution and any behavior delta from upstream in the
  commit message.
- Keep mechanical cleanups separate from behavior changes so backports and
  bisects stay reviewable.
- Do not update shared defconfigs, DT ABI, or userspace ABI for a board-local
  issue unless the blast radius is intentional and documented.
- Validate against the vendor integration surface: active DTS, firmware blobs,
  rootfs packages, out-of-tree modules, bootloader assumptions, and CI artifacts.
- For stable candidates, keep fixes small and bug-focused, and avoid risky
  feature/refactor payloads. A fix is only applied to a stable tree once it
  (or an equivalent) is in mainline.

## Commit And Review Notes

- Commit message should explain the problem, user/developer impact, and why the
  fix is correct. Do not narrate the diff.
- Include hardware, datasheet, log, reproducer, or regression evidence when it
  materially supports the change.
- Add `Fixes:` when the culprit is known. Add `Cc: stable@vger.kernel.org` only
  when stable rules are met.
- Run `scripts/get_maintainer.pl <changed-files-or-patch>` and check subsystem
  docs before choosing routing.
- Before final handoff, compare the final diff to this checklist and state any
  validation that could not be run.
