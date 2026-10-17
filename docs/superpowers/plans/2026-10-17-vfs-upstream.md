# VFS upstream integration implementation plan

**Goal:** Integrate the reviewed upstream corrections with the current dev branch,
preserve existing local fixes, and revert the SUSFS loader restriction requested
by the maintainer.

**Scope:** Existing VFS routing, ART/seqcount/SRCU, opaque/whiteout behavior and hm1
wire protocol remain authoritative. Upstream code is a reference, not a replacement.
The user approved the recommendations and requested implementation on 2026-10-17.

## Tasks

- [x] Preserve the original dirty tree in a recovery stash and external patch.
  Fast-forward dev and reapply the existing work. Resolve overlapping CI changes
  without dropping either the remote release work or local VFS tests.
- [x] Reverse f1ea5c9128c9a8475779a485ac6eb912661188db in its own commit,
  including installer/runtime guards, detector, tests and documentation.
- [x] Read the supplied bugreport without executing its contents. Correlate the
  actual `vfs load` entry with loader diagnostics; distinguish a userspace refusal
  from a kernel insertion failure. Do not infer ABI compatibility from vermagic.
- [x] Add failing production-function tests for hostile UID pagination cursors,
  then preserve unsigned indices and bounds at the kernel boundary.
- [x] Add fault-injection tests and propagate hook preparation errors before
  children become visible, preserving rollback and first-error cursor semantics.
- [x] Test RCU entry snapshots and directory lifetime across sleeping callbacks;
  preserve the separate SRCU reader contract and inode generation ownership.
- [x] Retain copied directory fops for open descriptors, pin their original owner,
  and account for Hybrid Mount's owner until unload.
- [x] Reuse credentials within a single directory traversal. Benchmark the
  adaptive Bloom candidate and keep a reproducible oracle, collision cases,
  memory figures and explicit userspace-only measurement limits.
- [x] Check the already integrated module.yaml, icon and release name mapping;
  do not create duplicate distribution metadata.
- [x] Run relevant C harnesses, Rust formatting/lints/tests and shell regressions.
  Compare native Windows failures with clean dev before calling them regressions.
- [x] Independently review the final combined patch and fix material findings.

## Decisions and verification boundaries

- The disabled SUSFS loader still leaves an object at a path inspected by the
  reverted detector. The detector gated `vfs load` and `lkm-load` before dispatch.
  This is a supported explanation for refusal, but the bugreport lacks the
  actual command's stdout/stderr and post-attempt kernel log.
- The default floating nightly toolchain fails to update on this workstation.
  Use the installed nightly-2026-08-10 for local Rust checks, without changing
  the repository's toolchain configuration.
- Full Android/GKI module compilation needs the DDK workflow. Never rewrite
  binaries/sources.txt to make stale binaries pass the provenance gate. Native
  extracted-function tests do not establish kernel ABI or device correctness.

## Final local result

The 98 native C cases pass. Formatting, Clippy, Android arm64 Rust check,
VFS control (18), runtime rules (24), xtask (16), and relevant shell tests pass.
Full native Windows workspace tests retain the same ten failures reproduced
in a fresh origin/dev export. Independent source review found no actionable
introduced regression.

## Remaining release verification

- [ ] Build the updated .ko files in the DDK workflow and run the source/prebuilt
  provenance gate against its actual outputs.
- [ ] Verify insertion and the supported VFS operations on the reported device;
  capture immediate command output/kernel logs if insertion still fails.
