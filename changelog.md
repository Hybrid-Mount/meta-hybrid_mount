
## v6.1.4


### <!-- 1 --> Features

- `webui` Animate module expansion



### <!-- 2 --> Fixes

- 修正文档中的描述和注释，确保语义清晰

- `module` 修正黑名单中的拼写错误

- Follow symlinks for overlay target metadata

- Preserve overlay staging and size shallow copies

- Fail closed on invalid boot config

- `webui` Fail closed without manager bridge



### <!-- 5 --> Miscellaneous

- Redesign SelectField & status page

- Reduce feedback latency




## v6.1.3


### <!-- 1 --> Features

- `webui` Show blacklist reasons in modules tab

- `config` Fall back to defaults on load failure



### <!-- 2 --> Fixes

- Fix umount all mountpoint Currently, only non-overlay mounts should be uninstalled.

- `webui` Localize mount target counts



### <!-- 5 --> Miscellaneous

- French translation fixes and enhancements

- `webui` Format pnpm lockfile

- French translation fixes and enhancements




## v6.1.2


### <!-- 4 --> Tests

- Skip long xattr probe when filesystem limit is reached

- Derive version assertion from package metadata



### <!-- 5 --> Miscellaneous

- Use tag as release package version

- Complete and fix Turkish (tr-TR) WebUI locale translations




## v6.1.1


### <!-- 1 --> Features

- Add APatch LKM fallback and module safeguards

- `scanner` Enforce module identity with validated ModuleId Introduce a ModuleId newtype as the single validation boundary and use it throughout config rules, blacklist, scanner records, mount tree, plan, and state snapshots while keeping the JSON/TOML wire format unchanged. Scanner now rejects duplicate declared module IDs instead of silently sorting, skips directories whose name disagrees with module.prop, and turns an unreadable module root into a fatal scan error instead of an empty list. Entry-level read errors are logged and skipped explicitly. Closes plan phase 2 (PR5) and gap G08.

- `mount` Roll back the whole pipeline through one transaction Register every successful side effect in MountTransaction: runtime temp dir, overlay storage, magic staging, intermediate overlay staging, overlay child and shallow mounts, magic bind/move targets, and the KernelSU try-unmount list. Rollback-only actions unmount targets deepest-first and are skipped on successful commit unless cleanup fails, in which case commit rolls back mounted targets as well. Pipeline rollback now verifies mountinfo subtree IDs against a pre-execution baseline instead of assuming targets were not previously mounted. AtomicBool-gated fault injection covers overlay Nth-target, magic, KSU commit, state save, mountinfo read, EBUSY unmount, and staging removal failures, with Linux mount-namespace tests. Closes plan phase 3 (PR8) and gap G13.

- `observability` Add low-overhead boot phase timing - Add an RAII PhaseTimer that logs status=ok on finish and status=aborted when an early return drops the guard. - Instrument startup/config/scan/plan/storage/overlay/magic/state/ rollback/cleanup in the mount pipeline with phase= on key failure logs. - Log aggregate plan metrics including MountTree::node_count. - Keep per-module and per-mount details at debug level.



### <!-- 2 --> Fixes

- Avoid self-named partition alias mounts

- Unify active mount targets and expand readme locales

- `config` Fail closed on corrupt config and persist atomically - Config::load_or_default now returns Result: a missing config file still yields defaults with a config_missing flag, while corrupt or unreadable files stop the pipeline with path context instead of silently falling back to Overlay mode. - Reuse sys::fs::atomic_write for Config::save; file and parent directory are fsynced, temp files are cleaned up on failure, and symlinked config targets are rejected instead of being silently replaced by rename. - Reject global default_mode=ignore during parse, patch and save; keep per-module/per-path ignore rules working. - Treat a corrupt module_blacklist.toml as an error (fail closed); a missing blacklist file remains a normal empty blacklist. - Move config tests to src/config_tests.rs via #[path] sibling module. - Record PR4 status in docs/RUST_AUDIT_PLAN.md (§8, G03/G04/G07). Verified with cargo fmt, clippy -D warnings, workspace tests (117 on host), stable 1.97 tests, and x86_64-unknown-linux-gnu / aarch64-linux-android checks including test-only compilation.

- `storage` Adapt ext4 block size for large images

- `webui` Preserve module version display

- `magic` Make executor failures transactional Return structured bind, move, symlink, whiteout, replace, and no-op outcomes from Magic Mount execution. Record only real mount targets as active while registering staging paths for rollback, and propagate direct-child failures instead of continuing with a partial stage. Treat failed read-only remounts and mount moves as errors, immediately unwind the just-created mount, and let the existing MountTransaction roll back earlier Magic and Overlay effects. Add rootless fake operations plus bind/remount/move/symlink fault gates, serialize process-global fault tests, and move the Linux executor matrix into exec_tests.rs. Closes audit plan PR9; Android real-device candidate validation remains a release gate.

- `mount` Exclude apex from managed partitions Prevent the metamodule from mounting the apexd activation tree and blacklist MoveCertificate, which manages its own certificate mounts. Refs #403

- `state` Derive is_mounted from mountinfo and expose boot diagnostics - scan.ret.is_mounted is no longer derived from the plan: the pipeline confirms executed targets against mountinfo, maps them back to modules through the shared mount tree, and adds symlink/whiteout-only magic modules from executor stats. - RunState now records state_load (missing/loaded/corrupt/io_error), failed_stage, rollback_status and leftover_mount_targets, so status can distinguish mount failure, clean rollback and incomplete rollback. - Failure paths rewrite scan.ret back to all-unmounted after rollback. - confirmed_active_mounts keeps executor attempt counts separate from the final mountinfo-confirmed active targets. - Add wire snapshot tests for modules/status/install-state/clear-errors and legacy-state deserialization coverage.

- `storage` Close Stage 4 overlay and ext4 audit gaps - G05: strict cleanup paths now propagate mountinfo probe failures and re-read mountinfo after detach; best-effort probing remains Drop-only. - G06: tmpfs to ext4 fallback verifies the tmpfs mount is gone and fails closed when unmount or verification fails. - G09: reset_image_files removes only the exact modules.img file, preserving user backups like modules.img.bak. - Add 64-layer boundary table tests (0/1/63/64/65/128) and a mountinfo failure injection test for overlay staging cleanup. - Add ext4 sizing tests for hardlinks, sparse files, symlinks and special entries, plus an e2fsck 0..=3 exit-code contract helper/test. - Keep the ext4 loop device in StorageHandle for explicit teardown detach and add bounded EBUSY retry with exponential backoff for loop attach/mount/detach.



### <!-- 3 --> Documentation

- `webui` Complete French localization



### <!-- 5 --> Miscellaneous

- Harden mount planning and tooling

- `mount` Add MountTransaction and Result mount probes Turn is_mounted into Result<bool> backed by a shared MountSnapshot query layer, with a best-effort helper reserved for Drop cleanup paths. Update the overlay, pipeline, and storage call sites so mountinfo failures propagate instead of being mistaken for unmounted targets. Add MountTransaction as the cleanup journal for mount side effects: reverse-order rollback, explicit commit/disarm/rollback, aggregated failure reports, Drop as last defense, and retainable actions that only commit(true) may keep for disable_umount. Wire it into intermediate overlay staging cleanup without changing mount execution order. Closes plan PR6/7 and gap G15; full pipeline integration follows in PR8.

- `errors` Introduce structured errors and subprocess runner - Add ErrorClass (transient/permanent/manual-recovery) with an exhaustive classify() match covering every Error variant. - Add IoError and ContextError backed by CausalError so mount/storage/LKM/ state boundaries translate io, rustix, procfs, serde and subprocess errors instead of pre-formatting Error::Msg strings. - Add a unified subprocess runner with bounded head+tail output capture, independent total and I/O drain timeouts, explicit exit-code policies and env-redacted Debug output. - Migrate e2fsck, mke2fs, getprop, insmod and ksud/apd production call sites. - Mark PR10 abandoned and PR11 implemented in the Rust audit plan.

- `deps` Converge on rustix and slim zip/tokio features PR2 (rustix convergence): - Replace extattr lgetxattr/lsetxattr with a two-step rustix xattr helper (length query + Vec::with_capacity + spare_capacity), covering long SELinux contexts. - Replace the hand-written libc::lchown FFI with rustix chownat AT_SYMLINK_NOFOLLOW. - Replace libc EINTR/EAGAIN/EBUSY/ESTALE and ELOOP/EBUSY constants with rustix Errno constants; migrate test unshare to rustix unshare_unsafe. - Drop direct extattr and libc dependencies. PR3 (build dependency slimming): - xtask zip: default-features=false with only deflate-flate2-zlib-rs, removing aes/bzip2/deflate64/lzma/zstd/xz from the build. - notify tokio: full -> rt-multi-thread plus a minimal-runtime test. - cargo tree: xtask 385->322 lines, notify 284->273 lines.

- French translation (#401) Co-authored-by: PifGadget92 <tiktok.ds@outlook.com>

- Remove stale audit plan and trim redundant tests




## v6.1.0


### <!-- 1 --> Features

- Add active mounts computation and enhance backend card styling

- Implement cloneAppConfig function and add tests; enhance config handling

- Add module blacklist support and enhance configuration handling

- Add module filtering support and implement swipe navigation

- Enhance overlay layer metadata handling and improve logging



### <!-- 2 --> Fixes

- Simplify overlay execution plan type

- `webui` Restore touch swipe navigation



### <!-- 5 --> Miscellaneous

- `blacklist` Add self-mounting modules

- Fix Turkish character rendering in Material 3 UI (#399) Fixes Turkish characters appearing thinner in the Material 3 UI by correcting the font variable reference. Tested on device.

- Adjust modules toolbar width and margin for better layout refactor: update swipe pager thresholds and locking logic for improved responsiveness




## v6.0.1


### <!-- 1 --> Features

- Implement shallow overlay directory handling and metadata cloning



### <!-- 2 --> Fixes

- Avoid duplicate partition symlink scans




## v6.0.0


### <!-- 1 --> Features

- `core` Implement Stage 1 foundation - Cargo manifest for the from-scratch rebuild - defs: shared paths and constants for the new architecture - errors: unified error type with From conversions - logging: RUST_LOG-aware init (logcat on Android, stderr on host) + panic hook - config: TOML schema + defaults + load/save/gen-config/show-config core with 10 unit tests

- `magic-mount` Implement Stage 2 magic mount engine - node tree (RegularFile/Directory/Symlink/Whiteout) with merge and builtin partition promotion rules - read-only module scan: module.prop, disable/remove/skip_mount, .replace, whiteout detection - mount execution on Linux/Android: bind, replace, tmpfs skeleton, mirror, read-only remount - KernelSU try-umount list integration via ksu crate (v4 line disabled, MNT_DETACH flush) - planner extension point: collect only modules/paths selected as magic - 11 new unit tests; verified on host, x86_64-linux and aarch64-android targets

- `overlayfs` Implement Stage 3 overlayfs and storage backends - fsopen overlay primary path + escaped legacy mount(2) fallback (v4.2.0 semantics) - >64 lowerdir staging split, mountinfo child-mount rebuild with immediate-unmount rollback - tmpfs staging gated on CONFIG_TMPFS_XATTR; ext4 loop image with mkfs.ext4/e2fsck/SELinux/nuke - KSU try-umount registration now deduplicated with ignore-partition rules (v4.2.0 semantics) - 17 new unit tests for escaping, layer splitting, child path calc, image sizing; verified host/linux/android

- `plan` Implement Stage 4 hybrid planner - read-only module inventory scanner (module.prop, state markers, system entries) - rule priority: path rule > module default_mode > global default_mode - one backend per path with explicit PlanConflict errors (cross-module and in-module) - overlay lowerdirs aggregated per partition, directory rules direct, file rules staged for shallow layer - magic module ids + path allowlist mapped to Stage 2 Selection - module-source immutability regression test; 18 new unit tests

- `cli` Implement Stage 5 CLI and mount pipeline - hand-written arg parsing (no clap); full command contract of plan section 4.2 - save-config --payload hex JSON with merge semantics (null clears module default_mode) - show-config / gen-config / modules / status / version / install-state / clear-mount-errors / emulated-soft-reboot - no-arg pipeline: config -> read-only scan -> plan -> overlayfs -> magic mount -> KSU try-umount commit -> snapshots - shallow file-overlay staging under run dir; magic stats and run/state.json snapshot - scan.ret module list reflects plan backend and config rules; 14 new unit tests

- `webui` Implement Stage 6 WebUI core with dual UI - Vue 3 + Vite + TypeScript + vue-i18n, Miuix default with on-demand MD3 switch - shared lib layer: types/constants/api (kernelsu.exec + hex payload)/api.mock/stores - Miuix and MD3 Status/Config/Modules/Info pages; path-rule editor, module search/filter/rules, mount-error clearing, reboot confirm - dynamic locale loading with en/zh complete; CLI contract adapted (config/status/install-state/clear-mount-errors) - lint, typecheck and production build all pass; dual UI chunks verified

- `webui` Migrate legacy locales and surface mount errors in scan.ret - migrate es-ES/id-ID/it-IT/ja-JP/ru-RU/uk-UA/vi-VN/zh-TW/tr-TR to vue-i18n message structure via semantic key mapping; removed-feature keys are dropped - language picker now exposes en, zh and the 9 migrated locales (includes tr-TR and id-ID) - scan.ret gains mount_error + suggest_ignore fields so both UIs can render the module page contract - 71 rust tests pass; webui lint/typecheck/build pass

- `release` Implement Stage 8 module scripts, xtask and CI - module scripts: metainstall (symlink-only partition handling), customize (wizard + config seed), metamount (boot entry), uninstall - xtask: webui build with MODULE_ID injection, cargo cross build, module.prop render, zip packaging, update.json, notify command - tools/notify: Telegram helper with topic 6/37, farming-style captions, skip without secrets - CI: build/release/lints/notify/license-header/auto-label/auto-blacklist/dependency-audit adapted to rehybrid-mount single flavor - update.json, changelog.md, cliff.toml; shellcheck clean; workspace fmt/clippy/test pass

- `build` Support arm64, armv7 and x86_64 packages

- `webui` Restore dual MD3 and Miuix interfaces Rebuild the Vue WebUI around the 4.2.0 mobile layout, unify dialogs and action controls, restore dynamic branding and contributor metadata, and align the backend config/status contract. Co-authored-by: YC酱luyancib <luyancib@qq.com>

- Align runtime mount behavior and WebUI Restore the dynamic module description and directional MD3/Miuix page transitions. Respect OverlayFS unmount registration settings and neutral mount sources for ignored partitions. Refresh compatible Rust, WebUI, and Actions dependencies while keeping TypeScript 7 isolated until the lint toolchain supports it. Co-authored-by: YC酱luyancib <luyancib@qq.com> Co-authored-by: dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>

- `webui` Add Miuix runtime status banner



### <!-- 2 --> Fixes

- `i18n` Align recovered Turkish commits with final architecture - tr-TR.json key structure aligned with final en message set (semantic migration restored) - README_TR.md rewritten for the ReHybrid-Mount architecture - README.md keeps Turkish language link recovered from the original commit - drop one-shot legacy migration script that referenced removed-feature key names - all 11 locale key sets are subsets of en; picker dynamically includes tr-TR and id-ID

- `ci` Grant packages read for dependency audit container

- `ci` Add cargo xtask alias used by workflows

- `ci` Use bash shell in container jobs and broaden triggers

- `ci` Build Android via cargo-ndk and install shellcheck on demand

- `xtask` Drop target arg from cargo-ndk build call

- `ci` Pin working Telegram notifier API Roll tgbot back to 0.46 because 0.47 serializes flattened multipart caption fields as JSON values, causing Telegram to reject the parse mode at runtime. Ignore the affected 0.47 line until upstream fixes its form serializer.

- `config` Accept legacy custom mount entries Ignore and omit the retired custom_mounts field during upgrades so otherwise valid legacy configuration does not fall back to defaults. Co-authored-by: YC酱luyancib <luyancib@qq.com>

- `mount` Restore reliable module snapshots Split whole-module overlays below partition roots, persist the scanned module list before fallible mount phases, and rebuild missing or invalid snapshots for the WebUI. Co-authored-by: YC酱luyancib <luyancib@qq.com>

- `webui` Simplify status and action controls Remove the redundant status compatibility card, keep refresh as an accessible icon action, center the Miuix module toolbar, and flatten the MD3 toast. Co-authored-by: YC酱luyancib <luyancib@qq.com>

- `state` Persist mount plan before execution Write the WebUI state snapshot immediately after planning so backend selections remain visible when a later mount fails. Reuse and atomically update the same state after successful execution without adding a daemon. Co-authored-by: YC酱luyancib <luyancib@qq.com>

- `mount` Restore reliable staged module mounting Restore v4.2-style prepared trees across managed partitions, use unpredictable transient mount roots, and persist actual APatch/KernelSU boot state. Format and audit ext4 staging images through the pinned pure-Rust fork, retain loopdev mounting, and invoke KernelSU NukeExt4Sysfs after successful ext4 mounts. Co-authored-by: YC酱luyancib <luyancib@qq.com>

- `webui` Show actual boot storage state Display the effective APatch/KernelSU mount source and distinguish retained staging paths from storage released after a successful boot. Co-authored-by: YC酱luyancib <luyancib@qq.com>

- `mount` Restore backend switching and status

- `storage` Restore reliable ext4 staging



### <!-- 3 --> Documentation

- Add ReHybrid-Mount implementation plan

- Record Stage 0-5 implementation progress in plan



### <!-- 5 --> Miscellaneous

- Init ReHybrid-Mount from scratch

- Add Turkish (tr) translation support. (#379) Since my native language wasn't available in the WebUI, it felt a bit unusual to use. That's why I decided to localize it into Turkish. I hope you accept it. Thanks...

- Add Turkish README localization. (#380)

- Add README_TR.md links to the two required sections (#381)

- `repo` Prepare v6 as the new dev line Point development and release automation at dev, remove obsolete backend-specific WebUI styles, restore the maintenance templates that remain useful, and document how legacy history and contributors are retained.

- `history` Preserve legacy dev contributors Connect the v4 development history as a parent of the rebuilt v6 line without restoring the retired daemon, Kasumi, flavor, or React code trees.

- Finalize Hybrid Mount branding Retire completed reconstruction scaffolding and redundant build or notification entrypoints. Refresh user-facing and architecture documentation while preserving stable runtime contracts and contributor history. Co-authored-by: YC酱luyancib <luyancib@qq.com>

- `mount` Unify backends on shared node tree Drive OverlayFS staging and Magic Mount execution from one backend-annotated tree, including symlink, replace, and whiteout semantics. Co-authored-by: Tools-cx-app <localhost.hutao@gmail.com>

- Fix mixed partition mount planning



# Hybrid Mount Changelog

## v6.0.0 (unreleased)

### Features

- 重构 Hybrid Mount：使用单一 Rust 二进制与 `kernelsu.exec` 直调 CLI，不再依赖 daemon / HTTP / SSE
- 混合挂载 planner：路径规则 > 模块 default mode > 全局 default mode，冲突显式报错
- Magic Mount：Node 树、`.replace`、whiteout、tmpfs skeleton、只读 remount 与 KSU try-umount
- OverlayFS：fsopen + 传统 mount fallback、超过 64 层时分段、mountinfo 子挂载重建与文件级 shallow layer
- storage 后端：tmpfs 与系统 mke2fs 格式化的动态大小 ext4 loop 镜像，挂载后接入 KernelSU ext4 sysfs nuke
- 运行临时目录改用无项目特征的随机名称，并按 `/mnt` → `/mnt/rw` → `/tmp` 回退
- 使用 `scan.ret` 与 `run/state.json` 连接启动流水线和 WebUI，不引入常驻 daemon
- Vue 3 双 UI：MD3（默认）+ Miuix，共享 lib 层与 11 个 locale
- module 安装脚本：symlink-only 分区处理与模块源目录只读约束
- xtask 构建打包、TG 通知及 CI/Release 连线
