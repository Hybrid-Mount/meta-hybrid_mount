# KernelSU Modules preparation and v6.2.3 release plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Prepare Hybrid Mount for KernelSU Modules and publish immutable v6.2.3.

**Architecture:** Preserve the existing module ID and Android build. Add catalog metadata on main and use the official validator and draft/upload/publish action. Generate the update URL from the published asset name.

**Tech Stack:** GitHub Actions, Rust xtask, module.yaml, PNG.

**Spec:** https://developers.kernelsu.org/docs/zh-CN/

## Global Constraints

- Keep `id=hybrid_mount` and `metamodule=1`; manifest uses `metamodule: true`.
- ZIP and manifest must agree; one module ZIP, valid int32 versionCode.
- v6.2.3 is stable, versionCode 602003999; enable immutability before publishing.
- Preserve self-hosted manager updates until the catalog is listed.
- User authorized existing Telegram release notifications.

## Review Focus

- Action renames the ZIP; update.json must use its asset-name output.
- PNG icon and description must exist on main, not only dev.
- Existing mutable releases cannot be made immutable retroactively.
- Tag must contain current VFS binary/source digests and all development fixes.
- Portal registration and review are separate from successful publication.

### Task 1: Catalog files

**Files:** module.yaml, icon.png, README.md, docs/KERNELSU_MODULES.md.

- [x] Add metadata matching xtask's generated module.prop; render existing icon at 512 square.
- [x] Run pinned official action with dry-run against a local test ZIP; inspect manifest and ZIP results.

### Task 2: Immutable release workflow

**Files:** .github/workflows/release.yml, CLAUDE.md.

- [x] Replace softprops release creation with pinned official module-release action.
- [x] Generate update.json using the action's normalized asset name; remove obsolete remote-repo instructions.
- [x] Verify YAML, release-version output, xtask tests, diff, and independent review.

### Task 3: Publish and verify

- [ ] Commit and push scoped changes; integrate dev into main using a fast-forward when possible.
- [ ] Enable repository immutable releases and read the setting back.
- [ ] Wait for main build and checks before pushing annotated v6.2.3 tag.
- [ ] Verify release immutable=true, stable, one ZIP, module.prop, SHA256, update.json URL, and version sync.
- [ ] Report portal installation/submission status separately from release publication.

## Execution notes

- Metadata, 512×512 PNG and pinned official dry-run passed; xtask 16 tests passed.
- Windows temporary-directory permissions required a repository-local TEMP for tests.
- Independent review found notification failure could block update metadata after an immutable release; moved notification after version sync.
- Minimum API is 26 overall, but the existing riscv64 binary targets API 35.
- Independent review completed; notification ordering finding fixed and verified. Windows full tests: 485 passed, 10 Linux/path-specific failures; main Linux CI is the release gate.
