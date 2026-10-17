# VFS upstream integration, 2026-10-17

This integration preserves the existing local parent-rule, inode lifecycle,
directory-entry and xattr fixes while applying the October upstream review to
the current dev branch. ART, seqcount, SRCU, opaque directories and the hm1
protocol are retained.

## Implemented behavior

- UID pagination keeps the wire cursor unsigned, checks the table bounds and
  returns an empty page for out-of-range cursors without indexing the table.
- Directory and superblock hooks propagate preparation failures before a child
  is published. Allocation and original filesystem module ownership are acquired
  before exposing copied operation tables.
- Lookup, iteration and revalidation snapshot proxy pointers under RCU. Directory
  references span operations that can sleep; RCU is released before calling the
  original filesystem. The inode-generation ownership contract remains separate.
- Copied directory fops declare Hybrid Mount ownership and pin the original
  filesystem owner. They remain allocated until module exit because an open FD
  can still point to them after inode operations are restored. This trades memory
  retention under inode churn for descriptor safety; it is not a general proof
  of safe module unload for every pre-existing VFS reference.
- Directory traversal reuses a per-call fsuid through actor filtering and virtual
  entry emission. Credentials are not cached globally or between calls.
- Cached directory reuse also checks backing-path identity, so a replacement
  with the same semantic flags cannot silently keep serving its old source.

## Performance and distribution

The reproducible [lookup benchmark](../tests/kernel/vfs_lookup_bench.md) compares
the production child search with adaptive Bloom candidates. It records userspace
measurements and memory overhead, not Android kernel speedups. Fixed64 and the
tagged-single fast path remain in production pending device profiling and
concurrent-update validation. A candidate microbenchmark alone does not justify
changing the live array publication contract.

The current dev branch already contains module.yaml, the PNG icon and immutable
release validation. These were checked against the official schema, declared
Android architectures/API and the generated ZIP name instead of being duplicated.

## Reported VFS insertion failure

The requested revert of f1ea5c9128c9a8475779a485ac6eb912661188db removes the
SUSFS object detector from installation, boot, `vfs load`, and `lkm-load`.
The supplied report contains a disabled SUSFS loader whose kernel object still
exists under the module directory. The reverted detector scanned this object
without honoring disable markers, so it could refuse a load before attempting
kernel insertion unless its KernelSU Next exemption succeeded.

The report does not include the actual load command's output or a subsequent
Hybrid Mount kernel rejection. Thus the userspace refusal is supported by the
installed layout and code, but no additional ABI or loader defect is proven.
The current kernel selects the android15-6.6 candidate; Android userspace version
is not the GKI tag. A differing vermagic alone is inconclusive because the loader
already handles an explicit vermagic rejection. No missing core symbol name was
established from the supplied symbol table. Unrelated Wi-Fi warnings and a panic
from a different historical kernel were not treated as Hybrid Mount failures.

If insertion still fails after installing a build containing the revert, capture
the complete `hybrid-mount vfs load --json` output and exit code, dmesg immediately
after the attempt, `vfs doctor --json`, `vfs guard --json`, and the installed
candidate's SHA256. Do not automatically clear a crash guard to gather these logs.

## Verification boundaries

- Native production-function harnesses exercise UID cursor extremes, allocation
  failures, hook ownership, reader snapshots, cached inode generations, source
  identity, parent-rule semantics and old-kernel xattr branches. They use
  controlled kernel primitives and do not verify the Linux/GKI ABI.
- Local Rust formatting and Clippy pass using the installed nightly-2026-08-10;
  Android arm64 `cargo check` passes. Native Windows tests reproduce ten failures
  also present in a clean origin/dev export (seven Linux mount-path assumptions
  and three path-separator expectations). The affected VFS control and runtime
  rule suites pass.
- Installer, boot lock, runtime hook and VFS integration shell regressions pass.
- No new prebuilt .ko was generated locally. The source/prebuilt digest gate is
  intentionally preserved and fails until the DDK workflow rebuilds the modules.
  Do not alter sources.txt by hand or release stale binaries as this fix.

No raw bugreport contents or personal device identifiers are included here.
