# Wear Package Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Deliver normal and ARM-only Wear packages through every build, release,
update and Telegram notification path.

**Architecture:** Share backend code and Material pages, but use separate frontend
entry points and isolated staging directories. Publish two explicitly identified
assets under one version/tag with separate update manifests.

**Tech Stack:** Rust xtask and notifier, Vue/Vite, GitHub Actions, Python/Node tests.

**Spec:** `docs/superpowers/specs/2026-11-25-wear-package-design.md`

## Global Constraints

- One release tag/version, module ID `hybrid_mount`, alternate installations.
- Wear userspace binaries: arm64 and armv7 only; normal retains four architectures.
- Preserve kernel module provenance and existing notifier dependency pin.
- Do not publish a tag/release or send live Telegram messages during validation.

## Review Focus

- Stale staging files must not leak architectures or frontend bundles.
- Both release assets must have validated identity and digest before publication.
- Normal/Wear update metadata must select the matching original filename.
- CI and notifications must include both artifacts, including manual dispatch.
- Notifier reuse must rebuild when notifier source changes; wire tests stay local.

### Task 1: Flavor-aware builds and frontend separation

- [x] Add failing tests for Wear architecture selection and flavor metadata.
- [x] Add normal/Wear/all flavor selection to xtask; isolate stage/output paths.
- [x] Add a Wear-only frontend entry; remove normal frontend watch detection.
- [x] Validate both frontend graphs and xtask tests.

### Task 2: Complete release and notification integration

- [x] Add regression tests for identifying and validating exactly two packages.
- [x] Update build/release workflows, metadata sync and release asset validation.
- [x] Add per-flavor TG captions and actual two-package multipart regression tests.
- [x] Update release documentation and run relevant workflow/notifier tests.

### Task 3: Final verification and delivery

- [x] Review final diff for single-ZIP assumptions and update-channel mismatches.
- [x] Run lint, type checks, frontend tests/builds, Rust tests and workflow checks.
- [x] Attempt required AOCI maintenance if tooling becomes available; report gaps.
- [x] Prepare reviewed implementation for the authorized dev commit/push; report CI separately.

## Execution notes

- Ruling: work in the existing dev checkout — the user requested dev delivery and
  its only unrelated untracked content is MapleSU-review, which stays untouched.
- Ruling: use `Hybrid-Mount_Wear-...zip` — the pinned catalog matcher only supports
  star/question-mark globs, so this preserves the normal catalog pattern safely.
- Independent source review completed; catalog matching and long Telegram caption
  fallbacks were corrected and covered by regression tests.
- AOCI MCP methods and a configured CLI are absent from this session. No AOCI
  semantic index entries or receipts were fabricated; maintenance remains pending.
- Browser screenshots use the actual Wear frontend and development mock data with
  2x pixel-density round masks; they do not establish device installation behavior.

Validation observed: WebUI 179 tests and type check pass; normal/Wear builds pass
with enforced module-graph separation; xtask 20 unit + 2 CLI tests and notifier
8 unit/wire + 2 CLI tests pass. Package metadata/catalog tests, release asset
validation tests, installer architecture tests, ShellCheck, actionlint and skill
link validation pass. Live tag publication and watch-device installation remain
outside this host verification.

Final Clippy checks passed for xtask and notifier with all targets and warnings denied.
Official prechecks use `*ount-*-*.zip` to preserve previous-release versionCode
comparisons when the release has two assets; the matcher regression test passes.
