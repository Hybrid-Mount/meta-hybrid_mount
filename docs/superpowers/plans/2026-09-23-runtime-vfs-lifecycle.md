# Runtime VFS lifecycle implementation plan

Goal: make late-load soft reboot and pure-VFS module hot operations share owned runtime state, serialized mutation, readback and recovery.
Architecture: boot-ID scoped persistent resource ledger, process-held operation lock, exact owned-rule reconciliation, explicit soft-reboot hook, and CLI-backed WebUI controls. Preserve module source directories and independent boot enablement. No daemon or kernel ABI change.
Spec: accepted design in the task conversation, 2026-09-23.

## Constraints
- Only pure-VFS modules may hot load/reload; unload uses saved ownership, not rescanned files.
- Never clear foreign rules or guess resource ownership from mount source alone.
- Persist intent before mutation; incomplete changes require explicit recovery, never report success.
- Initial release does not atomically switch batches or execute module lifecycle scripts during hot operations.
- Do not clear crash guards to force retries. No implicit provider loading for hot operations.
- Soft reboot must release owned VFS and actual mounts including overlays; verify mount identity and namespace.
- Reboot detection must distinguish late-load, ordinary root, and unknown/error.

## Tasks
- [x] 1. Add reboot-mode parser/bridge tests, fail closed on unknown KernelSU mode, preserve non-KernelSU reboot support.
- [x] 2. Add testable owned-rule transaction model, boot-scoped ledger, operation lock, and cleanup identity checks. Tests cover removed files, partial apply/rollback, foreign changes and stale boot IDs.
- [x] 3. Integrate boot capture, cleanup hook and safe repeated execution. Persist owned resources and VFS owners; prevent dirty/legacy state from silently rebuilding.
- [x] 4. Add runtime CLI status/load/unload/reload for pure VFS modules, plan conflict checks, updated runtime counts; tests cover mixed-backend refusal and ownership.
- [x] 5. Add two-theme WebUI runtime actions with explicit temporary behavior and action errors. Refresh status after each operation; no scripts or device reboot on hot actions.
- [x] 6. Run Rust, Android cross-check, shell, frontend tests and browser QA; review diff and document device-only validation limits.

## Progress / rulings
- Work in isolated codex/runtime-vfs-lifecycle worktree. User authorized implementation; proceed without further design approval.
- Frontend reboot detection can be implemented independently while backend lifecycle is developed.

- Completed host ownership/rollback/legacy/UID/readback tests, shell hook tests and both-theme browser matrix (12 combinations). Android/Linux test targets compile; rooted-device validation is not available in this workspace.
- Read-only module polling merges installed metadata without overwriting committed runtime state. Snapshot-only failures retain complete ownership in `syncing` for cleanup.
- Conservative release limitations are documented in docs/RUNTIME.md: hidden baseline descendant mounts may require physical reboot; external raw writers remain outside the operation lock; no atomic rule-batch switch or script replay.
