
## v6.2.3-rc.2


### <!-- 2 --> Fixes

- `runtime` Warn about untracked VFS rules instead of aborting the boot `check_untracked_vfs` failed the boot when any live VFS rule was not derived from the plan, which is the opposite of what the mount path does deliberately: `report_legacy` logs an unattributable live mount and lets the pipeline continue, because APatch and KernelSU both drop a metamodule script that exits non-zero, so an abort costs every module its mounts. A rule that was not placed by this plan is the same kind of ambiguity — it can be a leftover whose ledger is gone, or another tool's rule — and nothing needs to fail over it. Log each rule and continue, and keep returning the kernel's original uid list for the ownership ledger. Nothing later adopts these rules either: new rules are installed alongside them, a collision surfaces as a batch failure with its offset, and cleanup reconciles only the rules this ledger recorded. This host has no linker for any Rust target, so CI is the first compiler to see this change.

- `overlayfs` Skip a refused sub-mount instead of failing the boot `mount_overlay` rebuilds the pre-existing sub-mounts of an overlay root as overlays of their own, passing the child mount point itself as the last lowerdir. Some kernels reject such a layer. A module's own `post-fs-data.sh` bind-mounts content from `/data`, that bind is detected as a sub-mount of the partition the module overlays, and on the reported device the f2fs bind at `/my_product/media/theme/uxicons/hdpi` made overlayfs answer `filesystem on './theme/uxicons/hdpi' not supported`: `fsconfig create` failed with EINVAL, the legacy `mount(2)` fallback failed the same way, and the child loop returned that error. A returned child error aborts the entire overlay phase, so the pipeline rolled back every registered target and the metamodule exited non-zero — which costs every module its mounts (docs/RUNTIME.md). This is the same ambiguity `37b778fa` and `92ad95af` already downgraded to a warning. Log the failing sub-mount, count it and continue. The root overlay, which stays fatal so the transaction can still roll back, already serves that subtree's module content from the staged module layers, and the pre-existing bind hangs off the superseded mount, so copying it back is not possible either. `enable_next_child_overlay_mount_failure` fault injection covers the new path with a real child mount. Host tests pass except the ten `vfs::cli` path-separator and `runtime::mounts` mountinfo failures that fail on the unmodified tree on Windows too; the Linux-gated test needs CI.

- `magic` Register real file binds in the KernelSU try-umount list A magic file override without a tmpfs skeleton binds the module file directly onto the real system path, so per-app unmounting needs that path in the KernelSU try-umount list. The registration guard looked at the staging directory instead, with `self.umount && !self.work_dir_path.starts_with("/mnt")`. The staging root is `/mnt/<random>` on every Android boot, so the condition was always false and a direct file bind was never registered; only the directory mounts were. Decide from where the bind landed instead of where the staging root lives. `unmountable_target(umount, has_tmpfs, target)` yields the target only when the bind went onto the real path. With a tmpfs skeleton the file is bound inside the staging tree, which the owning directory mount later moves onto the real path and registers there, so an entry naming the staging path would outlive its mount and the kernel would replay it on every later umount for the rest of the boot. Host tests pass except the ten `vfs::cli` path-separator and `runtime::mounts` mountinfo failures that fail on the unmodified tree on Windows too; the Linux-gated test needs CI.

- `scanner` Stop a mismatched directory from reserving its declared module id The directory-name-vs-declared-id check ran after `declared_ids.insert`, so a directory skipped for a name mismatch still reserved its declared id. A later, correctly named `modules/<id>` directory then died on the fatal `Error::DuplicateModuleId` even though no duplicate module existed. Move the check before the insert. The regression test builds `root/shared` (id `shared`, name matches) beside `root/other_dir` (id `shared`, name mismatched) and asserts that exactly one module is listed, the one whose directory name matches.



### <!-- 3 --> Documentation

- Drop redundant reports and deduplicate provenance prose `docs/RUST_BACKEND_REVIEW.md` restated material `docs/ARCHITECTURE.md` and `docs/RUNTIME.md` already cover, and `docs/README_EN.md` was an 88 B stub pointing at the root README that nothing referenced. `module/vfs/README.md` repeated, divergence for divergence, the ledger that `module/vfs/src/PROVENANCE` records as the normative source, plus the GPL rationale that THIRD_PARTY.md owns. Keep the ledger in PROVENANCE and reduce README.md to a summary that points at it. Record the local scratch directory in .gitignore while here.

- `license` Attribute meta-magic_mount-rs and the vendored skills `src/magic_mount/mod.rs` documents the backend as behaviour-compatible with meta-magic_mount-rs `8b85c9e`, README.md and every locale credit it, and `webui/src/ui/miuix/components/StatusCard.vue` is derived from it — but THIRD_PARTY.md, which does list the comparable lkmloader, had no entry for it. Add one, record the vendored skill trees that THIRD_PARTY.md already pointed at, and qualify the "the WebUI is Apache-2.0" claim for the one file that is not. `GPL-v3` is not a valid SPDX identifier, so correct that header to the `GPL-3.0-only` the rest of the project uses.

- Resync the translated readmes and the skill references with the VFS backend The ten translated readmes still carried the pre-45402cab loader paragraph: they claimed the bundled VFS module is not loaded when no rule selects VFS, and that selection is an exact kernel-line plus Android/GKI match. Both are false. `src/vfs/mod.rs` loads the bundled module on every boot before planning, and `src/vfs/lkm_target.rs` falls back to other builds on the same kernel line. Resync them with README.md: the boot-logic paragraph, the loader paragraph (missing from all ten entirely), the Android userspace note, the WebUI note and the Runtime CLI note. Also record the VFS backend in CLAUDE.md, correct the LKM selection description in docs/ARCHITECTURE.md, and refresh the CLI list, module map and lifecycle helpers in the hybrid-mount-overview and hm-runtime skills.



### <!-- 4 --> Tests

- `overlayfs` Drive the refused sub-mount skip from a synthetic mount list The sub-mount test built a real tmpfs mount and expected the kernel to report it as a child of the fixture overlay root. `require_mount_namespace` unshares only the calling test thread, while the mount list is read through `/proc/self`, which resolves to the thread-group leader, so a test thread's own mount is invisible and the assertion failed in CI with "the sub-mount was not discovered". The sibling mount tests pass vacuously for the same reason. Extract the sub-mount rebuild loop into `rebuild_sub_mounts` and drive it from a synthetic discovered list, so the skip path is covered without a real mount or a private mount namespace. Production behaviour is unchanged: a sub-mount the kernel refuses is warned and skipped, while a root mount failure stays fatal and rolls the transaction back.



### <!-- 5 --> Miscellaneous

- `release` Stop shipping debug data in the package The release zip grew from 3.27 MiB to 6.40 MiB in two steps: v6.2.1 started shipping the prebuilt hybridmount modules, and v6.2.2 added the riscv64 binary. What made both steps so expensive is that nothing was ever stripped. The Rust binaries carried a full symbol table — 1152 KiB in the arm64 build, 1956 KiB in riscv64, which is 47% of that file. The kernel modules carried DWARF that dwarfs them: the 5.15 module is 640 KiB, of which 25 KiB is .text. Strip all three places: * `[profile.release] strip = "symbols"` in Cargo.toml. `debuginfo` would only recover 98 KiB of the ~4.7 MiB of symbol tables, because these binaries carry no DWARF at all. * `llvm-strip --strip-debug` inside the DDK build, so the committed modules and their list.txt stay a description of what actually ships, which is the invariant lints.yml checks. * the ten committed modules re-stripped in place, keeping .symtab, __versions, .modinfo and every run-time relocation section. Measured on a rebuild of the v6.2.3-rc.1 archive with the stripped artifacts: 6,728,704 B -> about 4,862,891 B, -1.78 MiB (-27.7%). Also pin the digest manifests to LF and mark *.ko -text, so a Windows checkout stops rewriting bytes that scripts compare verbatim.

- Validate the DSH skills and retire the license-header workflow `.dsh/validate-skills.mjs` was documented in `.dsh/README.md` but run by nothing, so skill frontmatter and relative-link drift could only be noticed by hand. Add it to lints.yml, whose job already runs node for the WebUI, and document the gate in hm-verify. Delete `license_header.yml`. It invoked hawkeye with `licenserc.toml`, which exists on no current branch — it and the `LICENCE_HEADER` template were lost in the `df3dc990` history-splicing merge — so the weekly job had failed five times in a row (exit 101, "cannot load config: licenserc.toml"). One hawkeye config can express exactly one license text and this repository has three (core GPL-3.0-only, WebUI Apache-2.0, kernel GPL-2.0-only), so a replacement gate needs several scoped configs, not just the missing file. Track the skill trees and the validator as well: they were untracked while `.dsh/README.md`, THIRD_PARTY.md and docs/ referenced them, so a fresh clone lost the MIT attribution targets. The vendored trees keep their own LICENSE and their frontmatter provenance. Fix the stale skill descriptions while here: hm-rust-lsp described a `web` profile that does not exist on this machine and a dsh-lsp-actions version that cannot install, and hm-ci still counted eight workflows and described a KernelSU repository step that is disabled.

- `license` Give the remaining files an SPDX identifier `clippy.toml`, `webui/vite.config.ts` and `webui/index.html` carried no license header at all, and the eight `webui/src/ui/md3/v420/*.css` files carried the Apache-2.0 prose header without the machine-readable identifier that the rest of the tree uses. Add it to each, in the block style `webui/src/ui/md3/icons.ts` already uses. `webui/pnpm-lock.yaml` stays as it is: pnpm regenerates the file whole, so a header there would be dropped by the next dependency bump.

- `vfs` Refresh the prebuilt hybridmount modules

- Deduplicate the mount, planning and VFS control paths Collapse the mechanical duplication found by the full-project review without changing behaviour: - pipeline: the five failure epilogues become `record_mount_failure`, which keeps the state persist, the unmounted-module snapshot and the returned error together so the WebUI summary cannot drift from state.json. - sys/faults: one `take(gate)` helper replaces four hand-written `#[cfg(test)]`/`#[cfg(not(test))]` pairs. - sys/process: `start_drain` replaces the duplicated stdout/stderr capture setup. - overlayfs/utils: one generic `retry_ebusy` over a small `Busy` trait replaces the `io::Error` and `rustix::io::Errno` variants. - overlayfs/overlayfs: one `bind` closure for the two bind-mount failure arms. - plan: `child_target` replaces three identical target builders, `MountPlan::vfs_module_id_strings` replaces four collect sites, and `defs::MOUNT_ERROR_REASON` replaces the repeated marker literal. - state/mount_tree/utils/config/errors: a shared mount-error directory walker, a delegated mount-tree collector, one xattr reader and one config loader, and the test-only error classifiers. - vfs: `paginate`, the version classification around `load_lkm` and the read-back closure in `Controller::mutate` are shared; the production-dead `apply_rules_with_policy` and a stale `#[allow(dead_code)]` are gone. Also cache `/proc/config.gz` in a `OnceLock` instead of gunzipping it twice per boot, and look the mount snapshot up in its id map instead of scanning every mount point. Retire the four assertions that stronger siblings already cover: `faults::gates_are_consumed_exactly_once`, `backend_tests::upstream_nomount_version_is_rejected`, `timing::timer_without_finish_drops_as_aborted` (asserted nothing), and the trailing `assert!(module.disabled)` in `runtime::policy::temporary_load_preserves_disabled_marker_semantics`.

- Run the emulated soft-reboot hook test tests/shell/runtime_hooks.sh was executed by no workflow; only shellcheck's glob parsed it. Run it beside the boot-lock gate. That is safe because module/emulated-soft-reboot.sh execs exactly the `runtime prepare-reboot` invocation the test's fake binary accepts.

- `skills` Vendor the rust-unsafe-review and rust-guidelines skills Vendor two upstream skill sets under .dsh/skills and record them in the skill README: - rust-unsafe-review (google/rust-skills, Apache-2.0): the upstream name is renamed to match its directory, and EVAL.yml, the testcases and the experimental tree are dropped. - rust-guidelines (microsoft/rust-guidelines, MIT): curated to the correctness, universal, ffi and checklist chapters because the rules already vendored from rust-skills cover the rest. Both carry a Hybrid Mount precedence section, and `node .dsh/validate-skills.mjs` reports 19 skills and 0 problems.




## v6.2.3-rc.1


### <!-- 2 --> Fixes

- `runtime` Report a pre-ledger mount snapshot instead of failing the boot A pre-ledger `run/state.json` records only the targets Hybrid Mount touched, never the mounts themselves, so a live mount on one of them cannot be attributed to Hybrid Mount. Rejecting that snapshot stopped the whole pipeline, and APatch/KernelSU drop every module of a boot when the metamodule script exits non-zero, so a device that upgraded from a pre-ledger version was left with nothing mounted: the platform's own `/product/overlay` sits on a target a 6.2.1 session had recorded, which was enough to fail every cold boot. The snapshot is now reported and left untouched, and a persisted ownership ledger from an earlier kernel boot proves the recorded targets are unreachable, so the check also stops firing after any physical reboot. The same age problem hid removed sources in the module list: `scan.ret` outlives the boot that wrote it, so a removed module stayed listed as mounted until something rewrote the cache. A removed source now stays listed only while this boot's ledger still records mounts or VFS rules for it, which keeps active ownership visible for an explicit unload and drops what a reboot already lost. - src/runtime/lifecycle.rs: reject_legacy_mounts -> legacy_mount_conflicts, plus legacy_snapshot_reachable for the boot-scope proof - src/runtime/boot.rs: report_legacy() warns instead of aborting, in start() and cleanup() - src/runtime/device.rs: expose boot_id() so the check can compare it - src/runtime/ledger.rs: owned_module_ids() reads overlay/magic/VFS ownership only, never the committed scan.ret copy - src/runtime/mod.rs: boot_owned_module_ids() backs the module query - src/state.rs: merge_module_snapshot() keeps a removed source only while the ledger still owns it - src/sys/fs.rs: stub sync_parent_directory on non-unix hosts - docs/RUNTIME.md: document both changes - tests: lifecycle conflicts/boot scope and the removed-source query Verified with cargo fmt --check, cargo clippy --workspace --all-targets -- -D warnings, a linux-target clippy run that also builds the boot/device/hot modules, and cargo test -p hybrid-mount (453 passed; the 10 failures are Windows-host path/mount differences that fail identically before this change).




## v6.2.2


### <!-- 1 --> Features

- `runtime` Add VFS hot mounting and safe late-load lifecycle Share boot-scoped ownership, operation locking, readback and recovery across boot mounting, pure-VFS module actions and KernelSU soft-reboot cleanup. Add module load, unload and reload controls designed for both WebUI themes, strict late-load reboot detection, live module snapshots and lifecycle docs.

- `build` Add riscv64 architecture support Restore riscv64 alongside arm64, armv7 and x86_64, which was dropped in 06bcf525. riscv64 is a Tier 3 Rust target: rustup ships no prebuilt std and cargo-ndk has no ABI for it, so xtask builds it straight through the NDK with `-Z build-std=std,panic_abort` against the nightly rust-src component. The NDK only carries a riscv64 sysroot from r27 onward and only for API 35. - xtask: record per-arch clang prefix, API level and cargo-ndk capability; keep riscv64 out of the cargo-ndk invocation and add compile_through_ndk - src/vfs/sys.rs: SYS_ADD_KEY = 217 for riscv64 (same as aarch64) - module/customize.sh: select hybrid-mount-riscv64 - lints.yml: add riscv64 to the android matrix with a build-std check step - rust-toolchain.toml: add rust-src - tests: xtask argv assertions and the installer branch case - docs: mention riscv64 and the NDK r27 requirement



### <!-- 2 --> Fixes

- `webui` Use rounded rectangles for error banner labels

- `vfs` Keep injected dentries off the hijacked filesystem's dentry ops Dentries created by hybridmount are added with d_add()/d_splice_alias() and never went through the hijacked filesystem's ->lookup(), so its private dentry data is not initialized for them: overlayfs' d_fsdata stays NULL. hybridmount_hijack_dentry_ops() still gave such a dentry the foreign dentry_operations table (a copy that only replaced d_revalidate, leaving ovl_dentry_weak_revalidate() reachable) and set DCACHE_OP_REVALIDATE / DCACHE_DONTCACHE without ever clearing the DCACHE_OP_WEAK_REVALIDATE and DCACHE_OP_REAL bits that d_set_d_op() had taken from the superblock. The VFS then called ovl_dentry_revalidate()/ovl_dentry_weak_revalidate() on it; both read dentry->d_fsdata and dereference oe->numlower at offset 0x10, which panics the kernel, and a negative injected dentry reached the same place through hm_d_revalidate()'s forwarding to orig_dops because ownership was decided from d_inode alone. Give the dentries hybridmount creates their own table, hm_owned_dops, with a d_weak_revalidate() of ours, clear every DCACHE_OP_* flag that table cannot serve (HASH, COMPARE, DELETE, PRUNE, REAL), and record ownership in the d_op pointer so negative injected dentries are recognized too and can never be forwarded to the filesystem they were injected into. Dentries adopted from a real ->lookup() still get a working table; when the filesystem has its own dentry_operations and there is no hm_iop to cache a copy in, both the table and the flags are left alone. Also correct the d_revalidate() calling-convention guards to 6.14, where the four-argument form replaced the two-argument one.

- `runtime` Stop reporting a removed runtime temp dir as retained RuntimeTempDir kept a single `cleanup` boolean, so remove_now() cleared the same flag keep() sets. Drop then read the cleared flag and logged "runtime temporary directory retained: ..., reason=disable_umount" for a directory it had itself just deleted, which reads as if disable_umount were on when it was off and puts two contradicting lines in the same boot log. Track the lifecycle explicitly (Remove / Retain / Removed) so Drop only reports a retention when a mount is really being kept alive beneath the directory, and cover both outcomes with unit tests.

- `storage` Withdraw the transient staging mount from the KSU umount list finalize_mount_setup() registers the ext4/tmpfs staging mount point with the KernelSU try-umount list, and teardown() then detaches that mount and removes its directory. The registration was never withdrawn, so the kernel kept replaying a path that no longer existed on every later umount for the rest of the boot: KernelSU: ksu_handle_umount: unmounting: /mnt/<session>/<staging> flags 0x2 withdraw_unmountable() removes the entry again (from REGISTERED_PATHS and from the kernel list through the crate's TryUmount::del), and teardown() calls it once the mount is confirmed detached. commit_unmount_list() now builds the kernel list from the still-registered paths, so withdrawing before that commit works just as well as withdrawing after it. A failed withdrawal stays a warning instead of failing the teardown: the mount is already gone.

- `runtime` Harden hot mounting and reboot cleanup Track exact KernelSU registrations and delete only owned entries during staging teardown, rollback, and soft-reboot cleanup. Install and verify UID isolation before hot rule publication, and reject conflicting rules across UIDs. Pin dentry parents and names during ref-walk revalidation, refresh all seven kernel modules, and detect markerless APatch through its live kernel interface. Add regression coverage and remove superseded implementation documents.

- `mount` Normalize magic mount target propagation Kernel clones inherit the propagation type of the mount they were cloned from, so a MS_BIND from a shared source leaves the target in that peer group. Magic Mount binds staging mirrors and module files this way, so every target added its own `shared:N` entry to the propagation table, while the OverlayFS targets created with OPEN_TREE_CLONE stay private. Apps that read mountinfo for environment checks saw a target set that was half grouped and half ungrouped, with freshly allocated group ids leaving gaps in the ids they had already collected. magic_mount_bind() now unsets the clone with MS_PRIVATE right after the bind, and the directory path repeats it recursively after the staging tree is moved onto the real target. MS_PRIVATE only affects that mount: the source keeps its own peer group, and the kernel releases the group id when its last member becomes private, so the ids stop being consumed. Normalization stays best-effort, but it is no longer silent: once the mount phase ends, the active targets are read back from mountinfo and reported when any of them still belongs to a peer group. mount_entries() and the boot log now carry the propagation type of each entry (`propagation=private|shared:N| slave:N`) so a change in the propagation table is visible from both sides.



### <!-- 5 --> Miscellaneous

- `vfs` Refresh the prebuilt hybridmount modules




## v6.2.1


### <!-- 1 --> Features

- `lkm` Embed symbol-aware loader and VFS candidate fallback

- `webui` Show tmpfs VFS and Nuke installation capabilities

- `vfs` Add the hybrid-mount vfs CLI Expose the hybridmount provider to userspace through a new `hybrid-mount vfs` subcommand that manages rules and isolated uids over the existing keyring channel: rule add/del/list/clear (including whiteout and opaque), uid add/del/list, plus version/doctor/load. The entry point parses arguments and prints help without touching the kernel; `hybrid-mount` with no arguments still runs the mount pipeline, and `lkm-load`/`vfs-doctor` keep their contracts. Usage errors exit 2 (new VfsCliUsage error), while every pre-existing command keeps exit 1. Move the foreign NoMount probe out of pipeline.rs into vfs/guard.rs, where it sits next to the provider classification it shares with the doctor, and reuse the read-only presence check instead of a second copy of it. Document the command contract in docs/VFS_CLI.md together with the confirmed implementation proposal.

- `webui` Show the VFS provider kind in the installation state Add a "VFS type" row to the About page's installation state in both UI styles, showing LKM for a provider registered by a loaded module and Built-in for one compiled into the kernel image, and "Not verified" when neither module table lists it. install-state gains vfs_type, classified read-only from /proc/modules and /sys/module/hybridmount by the existing ModulePresence logic, so a status query never turns into an insmod. It stays independent of vfs_supported, which still requires the key type to answer this boot, so "LKM" next to an unsupported VFS remains explainable rather than contradictory. The WebUI normalizer accepts only the two known kinds and downgrades everything else to unknown, so an older backend or corrupt payload cannot leak an unnamed string into the UI.

- `webui` Surface every startup problem as a status banner The status tab only rendered the first problem it could find, and several of them never reached the screen at all: per-module mount_error markers stayed on the modules tab, a foreign VFS provider was hidden behind vfs_supported, and a failed status load only produced a toast that disappears on its own. collectStatusErrors() now gathers all of them into one ordered list, and an empty list is what keeps the banner off screen, so a healthy device never sees it. Each entry carries an i18n code plus the raw value that belongs to that sentence, so both skins say the same thing in every locale. The covered cases are a failed status load, a failed module scan, the startup stage and reason with its roll-back state and left-over targets, a foreign or failing VFS provider, per-module mount failures with their reasons, and an unreadable startup snapshot. Both skins render it above the rest of the page with role="alert": md3 gets a new StatusErrorBanner component that replaces the narrower failure card, miuix keeps its status card and adds the same banner underneath. sysStore and moduleStore now record why a load failed instead of only firing a toast. VITE_MOCK_STATUS_ERROR=1 pnpm dev renders the banner for manual checks. The simulated failure lives in the dev-only mock module, which production builds never load, so a shipped WebUI can only report what the device actually said.



### <!-- 2 --> Fixes

- Show nuke backend accurately

- `vfs` Propagate spawn failures in the vfs CLI tests rust-lints failed on `cargo clippy --workspace --all-targets -- -D warnings`: the `run` helper in tests/vfs_cli.rs called `expect`, which the workspace denies. The helper is a free function in an integration test, so clippy's `allow-expect-in-tests` never covered it, even though the unit tests in src/** that do use `expect` pass. Return `io::Result<Output>` and let each test propagate with `?` instead of relying on a lint allowance that does not apply here.



### <!-- 5 --> Miscellaneous

- French translation enhancement (#478) Co-authored-by: PifGadget92 <tiktok.ds@outlook.com>

- `webui` Fix prettier formatting in info.vue The nuke summary line exceeded the print width, so the webui-lints job failed on `prettier --check`.




## v6.2.1-rc.3


### <!-- 2 --> Fixes

- `vfs` Probe on every boot and hide unavailable backend



### <!-- 5 --> Miscellaneous

- `release` Temporarily disable KernelSU mirror publishing




## v6.2.0


### <!-- 2 --> Fixes

- P2 high-priority security and stability fixes - HM-RUST-009: Add mountsource configuration validation * Prevent config injection via malformed mountsource values * Accept only known sources (KSU, APatch, overlay) or absolute paths * Add 3 new test cases for validation logic - HM-RUST-011: Implement stale temp file cleanup on startup * Auto-cleanup orphaned .tmp files from previous crashes * Prevent disk space exhaustion from accumulated temp files * Clear current process remnants + 24-hour-old files - HM-RUST-010: Fix mount_error marker cleanup to be file-only * Remove only marker files, never recursively delete directories * Prevent accidental data loss from unexpected directory structures * Add type checking and enhanced logging Tests: All 216 tests pass (+6 new) Security: 3 attack vectors blocked Stability: Improved crash recovery Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>

- Make runtime writes and boot guards resilient

- `webui` Align contracts and clean up Miuix components

- `config` Accept retired daemon setting during upgrades Preserve active configuration and module rules when daemon_startup_mode remains from an older release. Continue rejecting unknown fields and malformed values.

- `notify` Upgrade to tgbot 0.48 and migrate document uploads Upgrade tgbot to 0.48 and migrate single-document and media-group uploads to the new caption and conversion APIs. Preserve HTML formatting and topic routing, with offline request-construction regression coverage. All PR checks passed.

- `notify` Avoid quoted multipart fields in tgbot 0.48 Pin tgbot to 0.46 until its flattened multipart serialization is fixed. Replace Debug-only payload assertions with loopback HTTP tests for document and media-group uploads.



### <!-- 3 --> Documentation

- Record completed fixes and pending device validation

- Remove completed development log and retain pending validation



### <!-- 5 --> Miscellaneous

- Remove redundant backend helpers

- Align build documentation and maintenance tooling

- `webui` Update compatible dependencies and normalize lockfile Update the seven compatible WebUI dependencies and normalize the lockfile to repository formatting. WebUI lint, all 69 tests, type checks, production build, Android checks and dependency audits passed.




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
