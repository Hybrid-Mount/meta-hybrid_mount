
## v4.1.3


### <!-- 1 --> Features

- Update build and release workflows, remove WebUI build steps, and enhance kasumi feature handling

- Enhance module validation and improve overlayfs staging cleanup

- Enhance backup path handling, improve error logging, and update kernel config checks

- Enhance Kasumi feature support with kernel compatibility checks

- Add tmpfs xattr support checks and enhance Kasumi status handling

- Implement module blacklist functionality and enhance module handling

- Suggest ignore mode when mount_error modules contain shell mount commands Scan .sh files in modules with mount errors for mount/mkdir/touch/bind commands and display a hint in the WebUI suggesting the user set the module to "ignore" mode to avoid conflicts.

- Add issue template for module blacklist requests and implement auto PR creation workflow



### <!-- 2 --> Fixes

- Update kernel status checks to reflect compatibility changes

- Enhance kasumi feature handling and update version in Cargo.toml

- `Kasumi` Fix redirecting and hiding This is a empty commit to get the latest kasumi ko

- Update version code calculation in release workflow

- Improve error logging for status loading, config loading, and ksu bridge initialization

- Make cargo clippy happy

- Make cargo clippy happy again

- Correct typo in partition ignore constants



### <!-- 4 --> Refactors

- Clean up dead code, improve mount point collection, and enhance logging initialization

- Simplify mode retrieval and enhance module display logic



### <!-- 5 --> Documentation

- Add AGENTS.md guide for AI coding agents

- Update README files to include Cargo features and CI checks for all flavors

- Move README files to docs/ and use absolute URLs Move all 9 language README variants into docs/ directory. Replace all cross-file relative links with absolute GitHub URLs to ensure links work correctly from any context. Update Cargo.toml and AGENTS.md to reflect the new paths. Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>



### <!-- 7 --> CI / Tooling

- Only auto-close bug reports missing attachments, add URL detection



### <!-- 9 --> Other

- Add files via upload Indonesian Language

- Add Indonesian to language selector (#336) ### Motivation - The WebUI included a locale file `webui/src/locales/id-ID.json` but `id-ID` was not present in the `availableLanguages` list, so Bahasa Indonesia could not be chosen in the language dialog. ### Description - Inserted `{ code: "id-ID", name: "Bahasa Indonesia" }` into the `availableLanguages` array in `webui/src/lib/stores/uiStore.ts` so the Indonesian locale becomes selectable. ### Testing - Verified the change and locale presence using `git diff -- webui/src/lib/stores/uiStore.ts` and `nl -ba webui/src/locales/id-ID.json`, then committed the update with `git commit` (commands succeeded). ------ [Codex Task](https://chatgpt.com/codex/cloud/tasks/task_e_6a165edc5b2c8325bac2b9c862af1ab8)

- Update api.mock.ts to suit new feature




## v4.1.2


### <!-- 2 --> Fixes

- Update zip file naming conventions for consistency in package detection

- Remove tree filter in release checkout and sync missing locale keys - Remove filter: 'tree:0' from release job checkout to fix git-cliff changelog generation error - Add 19 missing i18n keys across es-ES, it-IT, ja-JP, ru-RU, uk-UA, vi-VN, zh-TW locale files (kasumi uname, cmdline, unload warning, etc.)

- Make clippy happy



### <!-- 4 --> Refactors

- Move build.rs module.prop to OUT_DIR and split KasumiTab into sub-components - build.rs: write module.prop to OUT_DIR instead of source tree; replace rerun-if-changed on .git directory with .git/HEAD; consolidate imports - KasumiTab: decompose 1332-line single component into index.tsx orchestrator and 6 focused section components (Lkm, Runtime, Identity, UserHide, Maps, Features), plus HeroCard, SectionShell, types, and utils

- Replace catch (e: any) with unknown and add .catch() to void calls - Add getErrorMessage() helper to safely extract messages from unknown - Replace all 12 catch (e: any) with catch (e: unknown) using the helper - Add .catch() handlers to 3 void async calls in App.tsx that lacked them

- Improve caption handling by introducing truncation functions

- Streamline configuration structs by removing redundant serde defaults



### <!-- 5 --> Documentation

- Replace hardcoded version badge with dynamic latest-tag badge Use shields.io github/v/tag endpoint instead of hardcoded 4.0.5 so the version badge always reflects the latest release tag automatically.



### <!-- 8 --> Maintenance

- Remove overlay fallback feature and related configurations - Eliminated the `enable_overlay_fallback` option from configuration files and documentation across multiple languages. - Removed associated fallback logic from the codebase, including functions and references in the executor and magic mount modules. - Updated README files in various languages to reflect the removal of the overlay fallback feature. - Adjusted WebUI components and localization files to remove references to the overlay fallback option.




## v4.1.0


### <!-- 1 --> Features

- Update summary in module.json to include 'Kasumi'

- Update version in README files to 4.0.5

- 添加对浅层覆盖的支持，优化挂载计划处理



### <!-- 2 --> Fixes

- Scope SELinux Guard to app zygote (#27) Kasumi Fix

- `umount` Skip pairip-protected lib partitions to avoid SIGSEGV KSU detaching the overlay over /system/lib*, /vendor/lib* mid-flight crashes pairip-protected APKs in libpairipcore.so. Filter those paths out of the umount queue and dedupe via HashSet::insert so repeat calls are no-ops.

- Handle module markers case-insensitively

- Make clippy happy

- 更新文档以更准确地描述构建版本的用途和功能



### <!-- 4 --> Refactors

- `notify` 重构 Telegram 通知消息排版 - 文件组不再附带独立 caption，三个文件干净排列 - 文件组后独立发送文字消息，包含标题、分支、产物数、commit 和日志链接 - 移除产物大小统计行

- `nano` 用 marker 机制替换 OverlayFS 白名单 - 移除 overlay_whitelist 配置项和 NANO_OVERLAY_WHITELIST 常量 - Nano 改用模块目录下的 overlay/magic marker 文件来判断挂载模式 - metainstall.sh 增加 nano 检测、mode marker 写入和分区列表扩展 - 更新 README 文档说明 marker-based 模式 - inventory 模块增加 module_mount_mode_marker 及测试

- `notify` 合并文件和文字为单条消息 Media group 第一个文件附带共享 caption，其余文件无独立文字

- 重构模块规则和处理逻辑，添加后代规则前缀支持



### <!-- 5 --> Documentation

- `readme` List supported languages

- `readme` Link available readme translations

- `readme` Add localized readme translations



### <!-- 9 --> Other

- Add Telegram notification helper for Hybrid Mount build artifacts - Introduced a new `notify` tool to send notifications via Telegram for build artifacts. - Implemented functionality to find ZIP files in the output directory and send notifications with details such as branch name, commit message, and artifact sizes. - Created a `NotifyRequest` struct to encapsulate notification parameters. - Added command-line interface to allow specifying a topic ID and event label. - Included tests for the ZIP file detection functionality.

- `notify` Apply cargo fmt




## v4.0.5


### <!-- 4 --> Refactors

- `tests` Improve test function names for clarity and consistency

- Reorganize imports for better clarity in main.rs




## v4.0.1


### <!-- 1 --> Features

- Enhance daemon and web UI with system info and runtime validation - Refactor server handling to use poll for improved performance. - Introduce system_info in API responses and update related interfaces. - Add URL validation to prevent malformed requests. - Improve error handling in config and kasumi stores with warnings for missing data. - Update App initialization to load UI and daemon concurrently.

- `App` Enhance app initialization phase tracking and loading UI

- `daemon` Add daemon_startup_mode config with persistent boot and webui toggle Add a daemon_startup_mode config option (on-demand / persistent) that controls whether the daemon starts on-demand via KSU exec or persists at boot via service.sh. Includes ping-first bridge optimization for faster reconnection. Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>

- Update TopBar to provide default app name and language title; add preconnect and styles in index.html

- Add clear mount errors functionality and UI updates - Implemented clearMountErrors API method in both mock and real API. - Updated AppAPI interface to include clearMountErrors. - Added clear-mount-errors command to the daemon command payload. - Enhanced Module interface to include mount_error property. - Added localization strings for mount error messages in multiple languages. - Introduced error indicators and banners in the ModulesTab component to display mount errors. - Added a button to clear mount errors, with loading state management.

- Enhance mount error handling and add clear mount errors functionality



### <!-- 2 --> Fixes

- `schema` Replace manual Default impl with derive macro for DaemonStartupMode

- `nuke` Skip nuke execution if KSU is not loaded



### <!-- 8 --> Maintenance

- Revert "feat(App): enhance app initialization phase tracking and loading UI" This reverts commit 08534f6f316b3f73ca2e09b95efdea4a5f8ff8ba.




## v4.0.0


### <!-- 1 --> Features

- Enhance ksu handling in module loading and add fallback check

- `daemon` Implement command handling and HTTP server for WebUI - Added `commands.rs` to handle various daemon commands including status, configuration, and Kasumi operations. - Introduced `http.rs` to manage the HTTP server for WebUI interactions, including session management and SSE support. - Implemented request validation and response formatting for daemon commands. - Created validation schemas in `validation.ts` for structured error handling and response parsing in the WebUI.

- `App` Ensure status is loaded during app initialization



### <!-- 2 --> Fixes

- Review and harden frontend-backend interaction - saveConfigToFile now includes kasumi and rules fields (was silently dropping) - Replace fastrand token generation with /dev/urandom CSPRNG - Use {err} instead of {err:#} in HTTP responses to avoid leaking filesystem paths - Remove Access-Control-Allow-Private-Network CORS header - Add /proc/<pid>/cmdline verification to PID file cleanup - Add config.toml.bak backup before overwriting config - Remove duplicate KasumiUnameMode enum, reuse schema::KasumiUnameMode - Replace inverted bool return with ConnectionAction enum in HTTP handler - Add typed DaemonCommandPayload discriminated union matching Rust serde tags - Add runDaemonCommand() bypassing JSON-in-shell-string round-trip - Preserve first error in bridge retry for debugging - Fix clearKasumiUname transaction order (clear runtime before config) - Fix ensureDaemonAwake TOCTOU race - Add proper types to RuntimeStatePayload stable fields - Rename parseHybridMountJsonOutput to parseDaemonJsonOutput - Clarify extractConfig fallback logic with comment



### <!-- 4 --> Refactors

- Remove module metadata from runtime entries and tests for cleaner payload handling

- `runtime_state` Simplify conditional check in save method

- `sysStore` Remove redundant systemInfo updates in handleSseUpdate



### <!-- 8 --> Maintenance

- Update .gitignore to include CLAUDE.md

- Pnpm format

- Make clippy happy



### <!-- 9 --> Other

- Refactor inventory discovery and planner modules for improved directory handling and performance




## v3.6.1


### <!-- 1 --> Features

- Add jq installation to Dockerfile and ensure jq is available in release workflow




## v3.5.6


### <!-- 1 --> Features

- `kasumi` Rename hymo -> kasumi

- Add nuke functionality before cleanup for ext4 storage mode

- Enhance module status update with kasumi_enabled flag



### <!-- 2 --> Fixes

- Update git remote URL for HymoFS source to Kasumi repository

- Enhance module build process to handle multiple .ko files

- `ci` Generate webui files before lint



### <!-- 4 --> Refactors

- Inline webui and remove config/status CLI bridge Move WebUI into the main repository, make runtime state read from daemon_state.json and configuration read/write from config.toml, and fold WebUI CI/dependabot into the main repo so the old submodule sync flow can be removed. Co-authored-by: 7a72 <11066204+7a72@users.noreply.github.com> Co-authored-by: 7a72 <git@zrlab.org> Co-authored-by: Anan <an@anatdx.com> Co-authored-by: KOWX712 <leecc0503@gmail.com> Co-authored-by: The Primal Pea <92656767+ThePrimalPea@users.noreply.github.com> Co-authored-by: ThePrimalPea <92656767+ThePrimalPea@users.noreply.github.com> Co-authored-by: Tools-app <localhost.hutao@gmail.com> Co-authored-by: UlasuNoka <ulasu.noka@gmail.com> Co-authored-by: YuzakiKokuban <heibanbaize@gmail.com> Co-authored-by: backslashxx <118538522+backslashxx@users.noreply.github.com> Co-authored-by: kuchazi <154660013+pkczc@users.noreply.github.com> Co-authored-by: lamprose <29279979+lamprose@users.noreply.github.com> Co-authored-by: luigimak <luigimak@hotmail.it> Co-authored-by: 由崎黑板 <94628337+YuzakiKokuban@users.noreply.github.com>

- Update build workflow to trigger on workflow_run and simplify conditions



### <!-- 8 --> Maintenance

- Revert "refactor: update build workflow to trigger on workflow_run and simplify conditions" This reverts commit 736332d6ccdfe59c6b6e38399a6191a2f2c8232c.



### <!-- 9 --> Other

- Make cargo clippy happy




## v3.5.5

### <!-- 1 --> Features

- Enhance kptools command handling and error reporting in nuke module

- Add mount topology command and handler for API

- Implement SaveConfigPatch struct and related functionality for config updates

- Enhance APatch KPM loading mechanism and improve error handling

- Refactor configuration handling to use ConfigSession for improved session management and patch application

- Add fast allocator feature and optimize release build settings for smaller binaries

- Refactor magic_mount to use MagicMountOptions for improved parameter handling

- `config` Refactor config loading to use load_default_config for improved error handling

- `cli` Add command to save all module rules and improve error logging

- `tests` Add unit tests for configuration and validation functions

- Add ModuleModeStats struct and integrate mode statistics into RuntimeState

### <!-- 2 --> Fixes

- Refactor config initialization in tests for clarity and consistency

- `core` Tighten config loading and module visibility Fail fast when the default config file exists but cannot be parsed, include blocked modules in the WebUI module list without affecting active mount scanning, and generate complete built-in partition metadata for the bundled WebUI.

- `config` Fail on invalid default config

- Fix clippy warnings

- Code quality improvements across multiple modules - Unify versionCode formula between build.rs and xtask (major*100000+minor*1000+patch) - Replace unwrap_or(0) with proper error propagation in xtask versionCode - Add Copy derive to MountMode, remove unnecessary .clone() calls - Add "type": "error" discriminator to ErrorPayload for robust frontend detection - Clean up unreachable code in conf/store.rs (bail! instead of read+unreachable!) - Add collision detection in get_mnt() with exists() check loop - Make init_logging() error handling explicit with .ok() - Add sync comments for cal_git_code between build.rs and xtask - Update webui submodule to b56861d

- Fix cargo fmt and clippy issues

- Update metadata URL to point to the correct repository

- Update default_mode in config.toml using sed for better configuration management

- Remove modules.img during ext4 cleanup Ensure finalize cleanup also deletes modules.img after tempdir removal when storage mode is ext4, so stale ext4 image files are not left behind. Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>

- Update read_c_buf to use iter().map for better clarity

### <!-- 4 --> Refactors

- Remove ignoreProtocolMismatch functionality and related UI elements

- Remove unused test cases and improve error handling in nuke and mount modules

- `core` Move dynamic partition discovery into runtime Extract partition discovery into a dedicated module, switch managed partition decisions to a shared runtime-driven source across sync, planning, kasumi, magic mount, topology, and system APIs, and update the bundled webui submodule pointer to the pushed runtime-driven partition display changes.

- Remove mimalloc dependency and update log handling for Android

- Simplify config update handling and improve logging for Kasumi

- `api` Replace serde_json::Value with typed structs, add structured error output and save-full-config - Replace json!() macros with typed Serialize structs: FeatureInfo, LkmPayload, MountStatsPayload, KasumiVersionPayload, SystemPayload, KasumiStatusPayload, MountTopologyPayload - Add ErrorPayload + print_json_error() for structured JSON error output on API commands, so frontend can parse errors from stdout - Add save-full-config CLI command that deserializes full Config instead of partial ConfigPatch, catching missing fields early - Add active_mounts and tmpfs_xattr_supported to SystemPayload - Remove unused paths from xtask generation: MODE_CONFIG, IMAGE_MNT, DAEMON_LOG - Add Clone derive to KasumiRuntimeInfo, Serialize derive to LkmStatus

- Refactor update desc APatch was supported module config(<https://github.com/bmax121/APatch/commit/05984a675a5effb171e49fa028d049f3e1243a1c>), so we can directly using apd/ksud module set

- Replace println! with scoped_log! for better logging in overlayfs utils

- Refactor update desc only using write file

- Update storage mode handling to use enum for clarity and type safety

- Replace hardcoded managed partitions with constant from defs

- Enhance error handling and logging in API and configuration loading functions

- Remove unnecessary blank line in RuntimeState implementation

### <!-- 7 --> CI / Tooling

- `android` Drop armv7 and x86_64 support

- Cache kasumi lkm builds

- Minimized size (#316) minimized size

### <!-- 8 --> Maintenance

- Update license headers to Apache License 2.0 across multiple files - Changed license information from GNU General Public License to Apache License 2.0 in the following files: - src/mount/kasumi/common.rs - src/mount/kasumi/compile.rs - src/mount/kasumi/mod.rs - src/mount/kasumi/runtime.rs - src/mount/kasumi/status.rs - src/mount/magic_mount/mod.rs - src/mount/magic_mount/utils.rs - src/mount/mod.rs - src/mount/node.rs - src/mount/overlayfs/mod.rs - src/mount/overlayfs/overlayfs.rs - src/mount/overlayfs/utils.rs - src/mount/umount_mgr.rs - src/sys/fs/file.rs - src/sys/fs/mod.rs - src/sys/fs/xattr.rs - src/sys/kasumi.rs - src/sys/lkm.rs - src/sys/mod.rs - src/sys/mount.rs - src/sys/nuke.rs - src/utils/mod.rs - src/utils/path.rs - src/utils/validation.rs - xtask/Cargo.toml - xtask/src/main.rs - xtask/src/zip_ext.rs

- Revert "feat: add fast allocator feature and optimize release build settings for smaller binaries" This reverts commit d4ded05dc6e0d3acedc85eb21f46e911895673a5.

- Run license header workflow weekly

- Fix typos, remove cert/key files, optimize string allocations, and standardize NDK version - build.rs: fix variable name typos (manjor→major, conut→count) - main.rs: replace panic!() with clean eprintln! + process::exit(1) - Remove cert.pem and private.enc from repository - Replace .to_string_lossy().to_string() with .into_owned() across 12 call sites - Standardize NDK version to r29 in release.yml (matching build.yml) Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>

- Add .claude/settings.local.json and .claude/ to .gitignore

- Format code for better readability in extract_module_id function

- Revert "refactor: refactor update desc" it will cause description no update This reverts commit f8c7b9049ed09dc7e65aa9a8e2907fcf0665adc0.

### <!-- 9 --> Other

- Panic on KernelSU late-load mode in rust entrypoint

- Make clippy happy

## v3.5.1

### <!-- 1 --> Features

- Implement SELinux context and ownership cloning for file operations

## v3.5.0

### <!-- 1 --> Features

- `lkm` Fallback to ksud insmod when finit_module fails on KernelSU When finit_module fails and the runtime environment is KernelSU, retry loading the Kasumi LKM via `ksud debug insmod`. Falls back through /data/adb/ksud then $PATH, and surfaces both failures on error.

- `nuke` Add strict verification for apatch nuke execution

- `xattr` Add support for legacy system file context in firmware paths

- `xattr` Refactor path resolution and context handling for managed partitions

- `xattr` Add function to determine if live context should apply to managed partitions

### <!-- 2 --> Fixes

- `lkm` Use 'ksud insmod' instead of 'ksud debug insmod'

- Improve live SELinux context application

- Collapse xattr let-chains for clippy

- Try fix GPU driver selinux is error

- Fix xattr

### <!-- 4 --> Refactors

- `xattr` Clean up formatting and improve readability in path resolution functions

- Improve code formatting and readability across multiple files

- Separate domain models and kasumi orchestration

- Enhance cleanup logic and add mounted path check

- Remove unused LiveContextCache and related functions

- `core` Simplify runtime state finalization

- `xattr` Remove unused lgetxattr import

### <!-- 8 --> Maintenance

- Remove unused test modules and associated test cases across various files - Deleted test modules and their corresponding tests from `user_hide_rules.rs`, `mod.rs`, `compile.rs`, `mod.rs`, `runtime.rs`, `tests.rs`, `utils.rs`, `file.rs`, `kasumi.rs`, `lkm.rs`, and `node.rs`. - Cleaned up code by removing commented-out test code and unnecessary imports. - This cleanup improves code readability and maintainability by eliminating redundant test cases that are no longer needed.

- Revert "Speed up CI by skipping kasumi LKM builds by default" This reverts commit 58e8b20423a6c1cdf5e344dd7686f300b9c06ca3.

### <!-- 9 --> Other

- Switch to GPLv2 licence

- Using hawkeye to manage licence

- Refactor sync and runtime finalization logic - Updated `perform_sync` to use `defs::managed_partition_names` for managed partitions. - Removed the `build_managed_partitions` function from `sync.rs` and replaced its usage. - Modified `build_runtime_state` to accept `ExecutionResult` instead of `MountPlan`. - Changed `collect_active_mounts` to use `ExecutionResult` for active mounts collection. - Introduced `managed_partition_names` and `managed_partition_set` in `defs.rs` for better partition management. - Refactored `build_managed_partitions` in `kasumi/common.rs` to utilize the new `managed_partition_set`. - Enhanced `collect_module_files` in `magic_mount/utils.rs` to accept `magic_modules` and `use_kasumi` parameters. - Implemented path normalization functions in `kasumi/compile.rs` for better path handling. - Updated `sync_dir` in `file.rs` to use a live context cache for improved performance. - Refined extended attribute handling in `xattr.rs` with better error logging and caching. - Removed deprecated functions in `kasumi.rs` to streamline the codebase.

- Simplify LiveContextSourceKind display

- Opt TMPFS_XATTR_SUPPORTED lock using AtomicBool, instead of OnceLock

- Refactor CLI handlers and remove legacy kasumi compatibility

- Split core API payload builders by domain

- Speed up CI by skipping kasumi LKM builds by default

## v3.4.7

### <!-- 1 --> Features

- `kasumi` Integrate Kasumi as third mount mode - New kasumi executor that drives the Kasumi LKM via ioctl rules (ADD_RULE / ADD_MERGE_RULE / HIDE_RULE / ADD_MAPS_RULE / HIDE_OVERLAY_XATTRS), with bidirectional src/resolved_src matching to keep module-side paths like /system/product working alongside the kernel's canonical form. - LKM lifecycle management: autoload/unload, KMI override, runtime probe via /proc/modules, and packaging of per-KMI kasumi_lkm.ko under module/kasumi_lkm/ via xtask (HYBRID_MOUNT_KASUMI_LKM_DIR). - New CLI surface: hybrid-mount kasumi {status,list,enable,disable, stealth,hidexattr,maps,hide-uids,mount-hide,statfs-spoof,uname, cmdline,fix-mounts,clear,release-connection,invalidate-cache} and hybrid-mount lkm {status,load,unload,set-autoload,set-kmi, clear-kmi}, plus user-hide persistence (hide add/remove/list/apply). - Planner / controller / runtime-state / finalization updated to treat kasumi as a first-class mount mode alongside overlay/magic. - Config schema extends with KasumiConfig (flags, uname/cmdline spoof, hide_uids, maps_rules, kstat_rules, mount_hide, statfs_spoof) and persists to /data/adb/hybrid-mount/kasumi.toml. - WebUI: new Kasumi tab with LKM card, runtime toggles, identity spoof, user-hide list, maps rules, and capability summary. Bottom-nav snowflake icon. kasumiStore uses /proc/modules as a fast probe: on LKM unloaded it synthesizes a fallback status that preserves the previous real config so the master toggle never flips off on unload. - CI: build.yml and release.yml now call build-kasumi-lkm.yml (matrix: 7 KMIs x arm64), download the .ko artifacts, and stage them via HYBRID_MOUNT_KASUMI_LKM_DIR before xtask build. - scripts/build-local.sh: local build helper with --kasumi-lkm-dir for dev iterations.

### <!-- 2 --> Fixes

- `kasumi` Drop redundant u64 casts in statvfs math to satisfy clippy

- `kasumi` Stabilise statvfs and default-config unit tests - statvfs_usage: widen via u64::from and silence the per-platform unnecessary_cast / useless_conversion lints instead of carrying target-gated code just for this helper. - kasumi_runtime_requires_mapping_or_explicit_feature: Config::default() turns stealth on; clear all auxiliary feature flags in the test so it actually exercises the 'no mapping, no feature' path.

- `kasumi` Remove redundant imports in compile and runtime modules

- `action` Update download-artifact action to v8

- `planner` Handle symlinks and improve error logging in generate_with_root function feat(utils): enhance collect_module_files to maintain partition structure fix(node): update symlink handling in Node implementation and add tests

- `module_status` Improve status description formatting in update_description function

- `kasumi` Align cmdline sync/clear behavior with upstream semantics

- `kasumi` Isolate runtime sync and harden compat

- `storage` Align tmpfs and ext4 selinux context

### <!-- 4 --> Refactors

- `kasumi` Unify config and tighten runtime behavior

- `kasumi` Reorganize use statements for better readability

- `build` Remove setup-build-env action and integrate KPM setup directly in workflows

### <!-- 9 --> Other

- Add Kasumi module with runtime and status management - Introduced a new Kasumi module in `src/mount/kasumi/mod.rs` to encapsulate functionality related to the Kasumi file system. - Implemented runtime management in `src/mount/kasumi/runtime.rs`, including feature toggles, runtime configuration synchronization, and application of mount rules. - Created a status management module in `src/mount/kasumi/status.rs` to handle operational checks and runtime information collection. - Added comprehensive tests in `src/mount/kasumi/tests.rs` to validate runtime behavior, feature toggles, and rule compilation. - Ensured proper logging and error handling throughout the module for better debugging and operational visibility.

## v3.4.6

### <!-- 1 --> Features

- `xtask` Call notify crate directly

### <!-- 8 --> Maintenance

- Split notify into separate repository

## v3.4.5

### <!-- 1 --> Features

- Add ext4 probe and post-check for APatch nuke flow

- Finalize APatch nuke KPM support

- Enhance ext4 sysfs handling by using function pointers for dynamic symbol resolution

### <!-- 2 --> Fixes

- Fix late mode check Signed-off-by: Tools-app <localhost.hutao@gmail.com>

- Fix panic

- Make kpm module compile in CI toolchain headers

- Collapse nested if to satisfy clippy

- Use APatch kptools for kpm nuke calls

- Only extract kpm assets on APatch

### <!-- 3 --> Performance

- `sync` Reduce repeated module tree traversal

### <!-- 8 --> Maintenance

- Remove extra kpm README and related doc entries

## v3.4.2

### <!-- 9 --> Other

- Fix installer notice confirmation blocking

- Improve mount planning diagnostics

- Refactor magic mount stats into context

- Make node traversal deterministic

- Polish executor naming and diagnostics text

- Unify logging format across runtime

## v3.4.1

### <!-- 2 --> Fixes

- `core` Resolve naming refactor build errors

- `recovery` Avoid state borrow conflict

- `planner` Split configured extra partitions

- `planner` Preserve real partition names for symlink targets

- `umount` Restore queued try-umount commit

### <!-- 4 --> Refactors

- `core` Split boot and command entrypoints

- `executor` Split overlay and magic handlers

- `storage` Split backends and ext4 setup

- `recovery` Split retry state and markers

- `core` Extract finalization workflow

- `core` Separate module description updates

- `inventory` Separate module presentation

- `core` Clarify controller and runtime names

- `startup` Rename boot recovery modules

- `naming` Rename entry, inventory, and fallback modules

- `naming` Rename status and finalization modules

- `umount` Drop extra /mnt cleanup and unify wording

### <!-- 6 --> Tests

- `planner` Add mount plan scenarios

### <!-- 7 --> CI / Tooling

- `submodule` Track webui on configured branch

- `release` Generate changelog with git-cliff

### <!-- 9 --> Other

- .github/workflows/update_webui_submodule.yml

## v3.4.0

Changes since v3.3.1:

- chore(submodule): update webui
- docs: add policy behavior matrix to Chinese README
- fix: complete P2 recovery retry and magic mount counter reset
- feat: complete P1 observability and tighten overlay fallback gate
- fix: narrow auto skip_mount attribution for magic mount failures
- workflow: update pnpm version
- adj: Add checks for unsupported root platform and late load
- fix: resolve clippy warning and update lockfile in release sync
- chore: clarify ELOOP fallback logging by condition
- feat: fallback symlink-loop overlay failures to magic mount
- chore(submodule): update webui
- chore: update cargo package
- chore(installer): switch notice prompt to English and add feedback channels
- chore: update license headers [skip ci]
- chore(release): sync version v3.3.1 [skip ci]## v3.3.1

Changes since v3.3.0:

- ci(release): install armv7 android rust target## v3.2.2

Changes since v3.2.1:

- build: pin nightly toolchain for rustfmt consistency
- chore(submodule): update webui [skip ci]
- ci: restrict webui updater workflow to dev branch
- fix: make cargo clippy happy
- metainstall: add support for hot install
- feat(core): auto detect manual mount scripts to prevent conflicts
- chore: update license headers [skip ci]
- chore(release): sync version v3.2.1 [skip ci]## v3.2.1

Changes since v3.2.0:

- chore: remove unused update_desc
- Revert "ci: promote only latest prereleases"
- Revert "ci(workflow): fix unrecognized 'secrets' context in if conditions"
- sync: sync magic mount updates from upstream
- adj: removed normalize_module_layout
- chore(deps): bump the crates group with 2 updates
- chore(tools): update notify binary [skip ci]
- chore(deps): bump rustls-webpki
- chore: bump webui submodule
- Update dependabot.yml
- Remove leftover EROFS module tool
- refactor: Fixed the issue with the size of the statistics
- BREAKING CHANGE: feat: The erofs is marked as deprecated
- ci: add caches to compilation workflows
- xtask: adj: adj target platform(<https://github.com/Tools-cx-app/meta-magic_mount-rs/pull/31>)
- Fix EROFS magic fallback and logging
- Fix EROFS empty remount handling
- ci(workflow): fix unrecognized 'secrets' context in if conditions## v3.1.6

Changes since v3.1.5:

- chore: fmt
- adj: Adjusting loop logic
- chore: make cargo clippy
- fix: Fixing layout errors
- chore: fmt
- feat: removed trait MountDriver in backend
- fix: fix glob rules again
- fix: make clippy happy
- fix: restore missing utils module and logging statements
- perf: eliminate O(N^2) tree cloning in magic mount and fix I/O safety
- fix: resolve critical mount bugs, ext4 inode exhaustion, and strict sync limits
- fix: fix error glob rules
- improve: Delete all imgs before mounting img.
- chore(deps): bump quinn-proto from 0.11.13 to 0.11.14 in /tools/notify in the cargo group across 1 directory (#249)
- feat(issue): add kernel version & hook type fields with auto-labeling
- chore: update license headers [skip ci]
- ci: delete unused workflow i18n
- ci: fix webui submodule path
- chore: add hybrid-mount-webui-md3 as webui submodule
- chore: remove webui folder for submodule migration
- chore(tools): update notify binary [skip ci]
- chore(deps): update dependencies in Cargo.lock
- chore(deps-dev): bump eslint in /webui in the crates group
- chore(deps): bump the crates group with 2 updates
- fix: make clippy happy
- refactor: remove random kworker camouflage feature
- feat: refactor logger system
- feat: add const_format dependency and refactor path constants
- feat: normalize module directory layout during sync
- chore(logging):logger tag to "Hybrid_Logger"
- ci: Simplify commit and push steps in workflow (#242)
- chore(tools): update notify binary [skip ci]
- chore(deps): bump aws-lc-sys
- chore(tools): update notify binary [skip ci]
- chore(deps): bump the crates group in /tools/notify with 2 updates
- chore(deps): bump zip from 8.1.0 to 8.2.0 in the crates group
- chore(release): sync version v3.1.5 [skip ci]## v3.1.5

Changes since v3.1.4:

- fix(core): expose full anyhow error chain and sanitize newlines for module.prop
- feat(core): catch daemon startup errors and display crash reason in module description
- feat(i18n): add step to delete old translation branch in workflow
- chore(deps-dev): bump globals from 17.3.0 to 17.4.0 in /webui in the crates group (#234)
- [skip ci]fix: update image source and correct binary name in README files
- chore(deps): bump actions/upload-artifact in the crates group
- New Crowdin translations by GitHub Action
- refactor: remove unused import of Show from ConfigTab component
- refactor: remove umount coexistence option and optimize ModulesTab performance
- Refactor store usage to uiStore and moduleStore across components
- feat: refactor application structure and implement new store management for configuration, modules, and system state
- fix(webui): resolve swipe stuck issue caused by requestAnimationFrame race condition
- feat: add Vietnamese translation for Hybrid Mount (#229)
- fix: fix error handling when scan modules (#228)
- feat(perf): implement UI performance optimizations
- chore(release): sync version v3.1.4 [skip ci]## v3.1.4

Changes since v3.1.3:

- fix(planner): correct module partition directory detection logic
- chore(deps): bump rollup## v3.1.2

Changes since v3.1.1:

- feat: add remote release step for KernelSU-Repo in workflow
- fix: correct typo in module summary
- chore(release): sync version v3.1.1 [skip ci]## v3.1.1

Changes since v3.1.0:

- daemon: using ksu's override.description api for description overriding
- chore(deps): bump ajv
- chore: remove unused fs import from mount.rs
- chore(tools): update notify binary [skip ci]
- feat: add branch name to Telegram notification and improve zip file detection
- feat: add webuiIcon parameter to module.prop generation
- feat: update launcher icon
- feat: add webui shortcut button
- chore(deps): update webui deps
- feat: removed useless read self mounts
- fix: fix typo
- chore: make cargo clippy happy
- docs: moved README to docs/
- refactor: abstract mount backend and storage implementations using traits
- fix: update refactored `fs` utility imports and function calls
- chore(deps): bump the crates group with 3 updates
- refactor: move fs module from utils to sys
- chore(release): sync version v3.1.0 [skip ci]## v3.1.0

Changes since v3.0.2:

- chore: add module.prop to gitignore
- fix: fix error handle logic
- feat: dropped folder check when umount
- chore: fmt
- feat: dropped zygisk check
- chore: removed module.prop
- feat: only use MNT_DETACH to umount
- workflow: fix workflow fmt
- refactor: ci&release logic
- feat: add overlay supported check
- chore: fmt && make cargo clippy happy (#215)
- fix: cargo fmt
- chore: fmt && make cargo clippy happy
- fix: fix nuke failed && fix ap no hidden ext4 loop
- feat(deps): bump && removed useless deps
- refactor: refactor mount_ext4
- fix: fix build
- feat: dropped poaceae subcommand
- chore(tools): update notify binary [skip ci]
- fix: fix build
- notify: make binary size less
- chore: fix gitignore
- refactor: refactor ignore partitions logic
- Change build command to use CI configuration
- Refactor:build_full function to handle CI and archs
- fix: fix error handling methods
- feat: dropped tmpfs setting
- opt: Optimize the logic of donnot umount
- feat: add ignore paths in umount
- feat: Make the errors of the fsopen operation more detailed
- fix: fix try umount flags
- feat: add umount successful log
- fix: correct modules name in some pathes
- chore(tools): update notify binary [skip ci]
- build: unify project naming to Hybrid-Mount and fix xtask syntax
- Revert "chore: rename package name in Cargo.toml"
- workflow: rename build.yml
- chore: rename package name in Cargo.toml
- xtask: refactor generate the module.prop
- chore(deps): bump tgbot from 0.41.0 to 0.42.0 in /tools/notify in the crates group (#212)
- chore(deps): update toml requirement from 0.9 to 1.0 in the crates group (#211)
- chore(release): bump version to v3.0.2 [skip ci]## v3.0.2

Changes since v3.0.14:

- chore: bump to v3.0.2
- build: inject release flag into webui constants and use it for logo display
- chore: fmt
- chore(release): bump version to v3.0.14 [skip ci]
