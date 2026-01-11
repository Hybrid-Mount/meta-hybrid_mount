## v2.0.3

Changes since v2.0.25:
* fix dead code warnings in executor and utils
* fix: make clippy happy
* refactor(core): use hybrid_mnt_dir for magic mount workspace
* fix(webui): fix storage status retrieval by reading state file directly
* fix: make clippy happy
* Revert "Refactor: Implement OverlayLayout strategy and enforce strict storage containment"
* fix: make clippy happy
* fix: add const remove by mistake
* Fix: remove unused `ensure_clean_dir` and resolve clippy lints
* Fix: prevent accidental deletion of modules directory
* chore: remove make ext4 during install
* 添加模块释放文件 (#119)
* chore: remove unused const
* fix: make clippy happy
* fix: make clippy happy
* Refactor: Remove deprecated overlayfs mount helpers
* Backport(it): Sync upstream storage logic and utils
* Refactor: Remove hardcoded OVERLAY_SOURCE and propagate mount source from config
* Refactor: Implement OverlayLayout strategy and enforce strict storage containment
* feat: support APatch by making KernelSU dependency optional
* refactor(storage): integrate overlayfs mount utilities
* chore: remove post apatch support
* fix: resolve dead code and clippy warnings
* fix(mount): clean unused import and fix OsStr conversion
* workflow: remove java setup
* fix(mount): migrate log to tracing and fix compilation errors
* backport(it): refactor mount architecture and update executor
* fix: make clippy happy
* chore: use tracing instead of logger
* refactor: unify logging system to tracing
* chore(deps): bump libc from 0.2.179 to 0.2.180 (#116)
* chore(deps): bump toml from 0.9.10+spec-1.1.0 to 0.9.11+spec-1.1.0 (#117)
* chore(deps): bump ksu from `f6b9700` to `47459ed` (#118)
* chore(release): bump version to v2.0.25 [skip ci]