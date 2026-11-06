# Partition mapping and VFS implementation verification

Date: 2026-11-06. Initial synchronized main/dev baseline: `d39c5925`.
Implementation through `22117faf`, plus the issue #511 regression test and refreshed prebuilts. Changes remain on local `dev`; this work does not publish a release or claim device validation.

## Host and integration checks

- WSL Ubuntu-26.10, root for real mount namespaces: `cargo test --workspace --locked` passes 628 tests: 598 core unit, 3 core CLI, 7 notification unit, 2 notification CLI, 16 xtask unit and 2 xtask notification CLI.
- `cargo clippy --workspace --all-targets --locked -- -D warnings` and `cargo fmt --all -- --check` pass.
- `tests/linux/partition_mount_boundaries.sh` passes real namespace cases for native nested/file mounts, complete staged publication, new module files, failure before publication and restoration. Directory removal and Magic Mount native-clone cases are included in the core suite.
- All six C production-function harnesses pass using WSL `cc`: `hook_transaction.py`, `inode_lifecycle.py`, `vfs_semantics.py`, `uid_cursor.py`, `reader_lifetime.py`, `child_rules.py`. Header dispatch includes eight version/member combinations. The native iterator regression fails with the old direct callback and passes through native VFS iteration.
- `tests/shell/vfs_setup.sh` and `tests/shell/vfs_xattr_feature.sh` pass, including generated in-tree Kbuild and same-version headers with/without `xattr_handler->__get`.
- WebUI: 171 tests, `vue-tsc`, ESLint/Prettier and production Vite build pass. No package lock change was needed.
- Device probe Python compilation and shell checks pass. These validate script syntax, not device behavior.

## Kernel build matrix and artifact identity

All seven supported DDK targets compiled and linked the current source successfully. Each image ran `make clean`, `make modules`, and `llvm-strip --strip-debug` in an isolated source copy. Before copying outputs into the repository, the builder compared all four input files byte-for-byte with the checkout.

| DDK tag (`ghcr.io/ylarod/ddk`) | Image manifest digest | Result |
| --- | --- | --- |
| android12-5.10 | `sha256:44c79fe1b443c332d2ce58627687e57448e27de2b9182466cabaf83d6c28492f` | PASS |
| android13-5.10 | `sha256:bc207e337e84128ae7962f99c144276223f8e5f1b96bc7ea8995a42e7b2cd8f5` | PASS |
| android13-5.15 | `sha256:5359ffd6ac64caebacd46955777bef32fd461b0f3b61b7d380e9847eb1c5e961` | PASS |
| android14-5.15 | `sha256:977948986ac6d78d3a940e3215a81efa26c4c4b7f0219b424a94fe2a9034e5f2` | PASS |
| android14-6.1 | `sha256:7f7d74a4405d72e78a800061662f1d03e0d792b9fc83cbea6e187ecf206436b8` | PASS |
| android15-6.6 | `sha256:fa881e2d59e23c806cca8dcc6b301d33ac482fff6baf85215f23208864a38b89` | PASS |
| android16-6.12 | `sha256:202deeabb05698b57ed6a4d4423a958631929f3e3942cfc57dc6854aa62fe349` | PASS |

The source digest over `hybridmount.c`, `hybridmount.h`, `Kconfig` and `Makefile`, using the existing workflow's digest-of-checksum-rows algorithm, is:

`d00156c10ea9d3d38f659cc72bce9e8572841a7fd11c287771d588ec475c8ed8`

`module/vfs/binaries/sources.txt` records that digest; `list.txt` records all seven output hashes. Fresh `vfs_sources_digest.sh`, `vfs_sources_digest_test.sh` and `sha256sum --check list.txt` all pass. The imported upstream baseline in PROVENANCE was not rewritten to impersonate the current build.

Additional fresh built-in object checks pass on android12-5.10 and android14-6.1 using `make -C "$KDIR" M=... obj-m= obj-y=hybridmount.o hybridmount.o`. These exercise object compilation without MODULE; setup-generated in-tree integration is separately covered by shell fixtures. They are not complete built-in kernel link or boot tests.

Temporary build-only Docker proxy and hosts overrides were removed and their absence checked. Docker and cached DDK images remain available for later builds.

## Review and retained decisions

A fresh whole-change review found three important gaps: raw escaped mountinfo paths, direct native iterator calls without native VFS locking, and native descendant capture beneath an already published opaque parent. Each received a failing regression and a correction. The full suite is green after the fix pass.

The following implementation decisions retain the approved behavior:

- Work directly on dev after the initial main/dev synchronization. The pre-existing managed worktree is left attached because the app would not archive its primary worktree.
- Execute the plans inline; task headings were normalized to `Task N` only for tracking. Device-only evidence remains explicitly outstanding.
- Repair the existing rustix xattr query regression as a prerequisite; run namespace checks with WSL root. Assertions were not weakened to pass on a restricted host.
- Keep concrete OS error strings for partition-probe diagnostics; absent paths stay absent. Conflicted existing partitions are skipped with errors and do not invalidate unrelated partitions.
- Prepare complete overlay subtrees before one recursive publication, preserving new module entries beside native child mounts. Do not accept partial publication after a child failure.
- Omit empty diagnostic fields from legacy module JSON and make them optional in frontend types, preserving older wire snapshots.
- Retain superblock/module lifetime protection: no reproduced ownership argument supports deleting it, and there is no equivalent reclaim workqueue to transplant. Host harnesses use the available `cc`; the DDK builds use their actual Clang toolchains.
- Independent partition work proceeded while kernel validation ran. This did not waive source/binary matching or device gates.

## Outstanding device evidence

No Android device startup, Iconify runtime retest, KASAN run, forced unmount or shutdown test was executed. Held-directory UID/CWD transitions and concurrent source/UID replacement probes are provided but have not run on a device. Source identity remains object-pinned; no automatic rebinding semantics were introduced. See the [lifetime report](2026-11-05-vfs-upstream-lifetime.md), [source identity report](2026-11-05-vfs-source-identity.md), [held-directory report](2026-11-06-vfs-uid-directory.md), and [pre-release investigation](2026-11-06-prerelease-511.md).

Host correctness, all seven module builds and artifact consistency are established here. These results do not close the outstanding device validation gates.
