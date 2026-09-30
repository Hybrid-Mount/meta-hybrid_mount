---
name: kernel-development-skills
description: 改 Hybrid Mount 的内核侧 C 源码（module/vfs/src/hybridmount.c、module/lkm/src/nuke.c）时用：驱动 API 与生命周期、Kconfig/Makefile、调试与追踪、KUnit/kselftest、patch 与回归排查的清单和脚本。
whenToUse: 修改随仓库维护的内核模块源码、排查内核侧 probe/生命周期问题、判断某个内核改动该怎么验证时。
license: MIT
metadata:
  version: "0.3.0"
  vendored:
    source: https://github.com/UtsavBalar1231/kernel-development-skills
    commit: cc513486979618140f2c9e6e31bd20530fb5e0e3
    vendored: "2026-09-30"
    upstream-path: plugins/kernel-development-skills/skills/kernel-development-skills
    trimmed:
      - agents/openai.yaml (Codex 专有)
      - references/lapis-rk3576.md (RK3576/Lapis 板级，与本仓库无关)
      - scripts/check_lapis_guardrails.py (同上)
compatibility: Bundled scripts require python3 (3.9+); content targets any Linux kernel tree.
---

# Kernel Development Skills

## Hybrid Mount precedence (read before applying any rule)

These rules are generic Linux-kernel defaults. This repository's own contracts win wherever they differ. Verify against `CLAUDE.md`, `module/vfs/README.md`, `module/vfs/src/PROVENANCE` and `.dsh/skills/hm-vfs/SKILL.md` before acting.

- **Read `hm-vfs` first.** It carries this repo's hard constraints: the load order, the boot-guard breaker, the GKI target matrix, and the fact that VFS must be loaded **before** planning.
- **The kernel C here is a maintained fork, not a driver you are authoring.** `hybridmount.c`/`hybridmount.h` come from NoMount; `nuke.c` comes from Mountify. Keep `MODULE_LICENSE("GPL v2")`, the upstream author attribution, and the `HYBRIDMOUNT_MAGIC_SIG` ↔ `protocol::MAGIC` byte-for-byte agreement. Do not restructure upstream code to satisfy a style rule.
- **You almost never compile it locally.** The `.ko` files are built by `.github/workflows/kernel-module.yml` in `ghcr.io/ylarod/ddk` images for 7 GKI targets, aarch64 only. Changing `module/vfs/src/**` makes the `sources.txt` digest gate fail **on purpose** until the rebuild commit lands — see `hm-vfs` and `hm-ci`. Never hand-edit `sources.txt` to make a commit green.
- **Board/devicetree material does not apply.** This module ships no DTS, no defconfig and no vendor board tree; the devicetree and `lapis-rk3576` guidance upstream has was removed here for that reason.
- **Rust is the caller.** Behaviour that looks like a kernel bug is often the Rust side not reading back state (`src/vfs/control.rs` writes then re-reads). Check both before blaming the module.

## Overview

Use this skill to make kernel changes with the same discipline expected by upstream Linux and vendor/LTS kernel trees. Start from local truth, keep patches reviewable, choose validation based on risk, and never bypass repo-specific build rules.

## Start Here

1. Read the nearest `AGENTS.md`/`CLAUDE.md`/repo guidance before touching files.
2. Inspect current state first: `git status --short`, relevant `Documentation/`, `MAINTAINERS`, Kconfig/Makefile, DTS/bindings, and existing subsystem patterns.
3. Determine the work type and load only the needed reference:
   - Patch/review/upstream/stable/regression: `references/kernel-workflow.md`
   - Debugging, tracing, CI, tests, sanitizers: `references/debugging-testing.md`
   - Driver authoring/API choices/lifetime: `references/driver-api-cookbook.md`
   - Driver review/maintenance/backports: `references/driver-review-checklists.md`
   - DTS, bindings, Kconfig, subsystem routing: `references/device-tree-drivers.md`
   - Advanced tools, fuzzing, trace pipelines, patch tooling: `references/advanced-tooling.md`
   - CI failures, KernelCI/LAVA/TuxSuite, regression reports: `references/ci-regression.md`
   - Finding local docs: `references/doc-routes.md`, or generate and query `assets/kernel-doc-index.jsonl` (see Local Documentation below)
4. Make the smallest coherent change that matches the subsystem's existing style.
5. Verify with the narrowest meaningful checks first, then broaden when the blast radius grows.

## Kernel Change Loop

1. **Scope**: identify affected subsystem, public ABI, DT ABI, hardware behavior, and whether userspace or firmware depends on it.
2. **Source of truth**: prefer local docs and code over memory. For board work, compare schematic/datasheet -> active DTS -> binding -> driver -> config -> runtime tests.
3. **Patch shape**: split mechanical cleanup, binding, driver, DTS, config, and tests into separate reviewable commits unless the repo explicitly wants one local commit.
4. **Implementation**: follow existing APIs and locking/lifetime patterns. Do not invent helpers unless they remove real duplication or make failure paths safer.
5. **Validation**: run style/static checks, targeted build, tests, boot/runtime checks, and log review appropriate to the affected subsystem.
6. **Review**: inspect diff, commit message, tags, maintainers, and regression/stable implications before declaring work complete.

## Technical Driver Work

For driver creation or non-trivial driver edits, load `references/driver-api-cookbook.md` before choosing APIs. It covers probe/remove structure, `devm_*`, `dev_err_probe()`, MMIO, regmap, GPIO descriptors, regulators, clocks, resets, IRQ/workqueue context, DMA, runtime PM, subsystem selection, and deprecated APIs.

For driver maintenance, review, regression fixes, or backports, load `references/driver-review-checklists.md`. It turns the cookbook into pre-edit, review, validation, and vendor/LTS checklists.

## Non-Negotiables

- Do not edit generated, vendor-imported, or upstream-owned files when a board/subsystem override exists.
- Do not change kernel/userspace ABI casually. Treat sysfs, ioctl, netlink, uapi headers, and devicetree bindings as compatibility surfaces.
- Do not silence warnings, probe errors, or test failures without proving they are unrelated.
- Do not claim a kernel build, test, boot, or debug fix is complete without fresh command output or runtime evidence.
- Never invoke kernel `make` directly to produce the shipped `.ko` files: they are built by `kernel-module.yml` through DDK images. `.dsh/skills/hm-vfs/SKILL.md` documents the only supported rebuild paths.

## Common Verification Matrix

| Change | Minimum checks |
| --- | --- |
| C driver logic | `scripts/checkpatch.pl --strict`, focused compile, relevant KUnit/kselftest or runtime probe/log test |
| Kconfig/defconfig | dependency review, olddefconfig through repo wrapper, diff of resolved config |
| Devicetree binding | binding schema check, compatible documented before DTS use |
| DTS board change | DT binding check, targeted dtbs build, boot log/probe/sysfs evidence on hardware when available |
| Debug/test-only change | confirm it is gated, low overhead, and disabled unless intentionally enabled |
| Stable/regression fix | prove culprit, add `Fixes:`, `Link:`/`Closes:` as applicable, evaluate `Cc: stable@vger.kernel.org` |
| Vendor/LTS backport | document upstream source, conflict resolution, and behavior delta from upstream |

## Bundled Scripts

Use scripts when they save repeated reasoning or catch local footguns. Paths
are relative to this skill's directory:

```bash
python3 scripts/triage_kernel_change.py [changed paths...]
python3 scripts/check_kernel_api_patterns.py [changed paths...]
```

`triage_kernel_change.py` emits a path-based checklist with references and likely checks. `check_kernel_api_patterns.py` is advisory only; use kernel-native tools and review judgment for final decisions. Upstream's `check_lapis_guardrails.py` was removed together with the rest of the RK3576/Lapis material, which this repository has no use for.

## Local Documentation

Prefer local docs in the checked-out kernel because vendor trees can differ from current upstream. `references/doc-routes.md` holds curated, stable routes. The per-checkout artifacts `assets/kernel-doc-routes.md` and `assets/kernel-doc-index.jsonl` do not exist until generated; create or refresh them from the kernel root (paths relative to this skill's directory):

```bash
python3 scripts/index_kernel_docs.py \
  /path/to/kernel-root \
  assets/kernel-doc-routes.md \
  --jsonl assets/kernel-doc-index.jsonl
```

Do not load the full JSONL index into context. Query it with `rg` or `jq`, then open only the relevant kernel docs.
