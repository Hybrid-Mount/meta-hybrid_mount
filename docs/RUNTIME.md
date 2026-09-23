# Runtime mounting and late-load reboot

Hybrid Mount shares one ownership ledger and operation lock between boot mounting,
KernelSU late-load (jailbreak) soft reboot, and pure-VFS module hot operations.
This implementation needs rooted-device validation before being treated as stable.

## Commands

Run `/data/adb/modules/hybrid_mount/hybrid-mount` with:

| Command | Behavior |
| --- | --- |
| `boot` | Run the boot pipeline once per recorded generation; a repeated completed hook is a no-op. |
| no arguments | Run the pipeline only from clean state; refuse layering over live owned resources. |
| `runtime status` | JSON capability, generation, and module active/eligible state. Unsupported or dirty state includes a reason. |
| `runtime load ID` | Apply a pure-VFS module for this session. |
| `runtime unload ID` | Remove that module's recorded VFS rules, including when its source files are gone. |
| `runtime reload ID` | Rescan and re-register its rules, refreshing kernel-pinned source paths. |
| `runtime prepare-reboot` | Verify and release owned VFS rules, isolation UIDs and real mounts before rebuilding. |
| `emulated-soft-reboot` | Compatibility alias for `runtime prepare-reboot`. |

Successful hot actions return `{"ok":true,"generation":N}`. Failures exit nonzero;
callers must refresh runtime and module status even on failure. Both WebUI themes
provide session load/unload/reload controls. Module enablement for the next boot is
independent: loading a disabled module does not delete its `disable` marker.

Hot operations require an already available supported HM VFS provider and clean
crash-guard state. They do not load a provider, replace module directories, run
module scripts, restart applications, or make a batch atomically visible. Mixed
Magic/Overlay modules, skip-mount/removal/blacklist entries, and paths overlapping
live real mounts are refused. Reload/load rescan configuration; unload uses saved
ownership. An active module may remain unloadable even if its new configuration
is no longer eligible for reload.

## Late-load and soft reboot

The WebUI validates the root-manager markers and parses exactly one `late_load`
boolean from `/data/adb/ksud debug info`. Missing, malformed, conflicting, or failed
KernelSU detection stops the reboot instead of guessing. Explicit APatch and
confirmed non-late-load KernelSU retain normal reboot behavior.

For late-load KernelSU the WebUI runs `runtime prepare-reboot` **before**
`/data/adb/ksud soft-reboot`; failed preparation prevents the reboot command. The
module also supplies `emulated-soft-reboot.sh` for external ksud callers. Some
ksud versions log hook failures and continue: Hybrid Mount's dirty ledger then
refuses a new pipeline, preventing a second layer of mounts. Use the WebUI path
when failure must stop the whole soft-reboot sequence.

The operation lock is held by a live process; its file is never removed. Kernel
boot ID and PID 1 mount-namespace identity scope `run/runtime.json`. A physical
reboot invalidates saved mount IDs; a soft reboot keeps them until cleanup.
The old `/dev/hybrid_mount_single_instance` directory is only removed as a legacy
marker and is no longer the synchronization mechanism.

## Ownership, recovery and limits

- VFS transactions validate owned records, reject foreign overlaps, record intent,
  apply changes, read back, and attempt scoped rollback. They never clear all rules.
  Reload re-registers unchanged path strings because the kernel pins source inodes.
  Conflicting paths are rejected across UIDs too: the kernel's directory lookup
  topology does not isolate same-name entries by UID. Hot load/reload installs and
  verifies newly requested isolation UIDs before publishing file rules; existing
  external isolation stays unowned. Failed operations undo only their added UIDs,
  and retain isolation if rule rollback cannot be verified.
- Real mounts are recorded by exact execution targets and new mount IDs relative
  to the pre-operation snapshot. Cleanup verifies namespace, ID, device, type,
  source, baseline stack and the visible mount ID; it includes Overlay child mounts
  and Magic mirror bindings and detaches deepest first.
  KernelSU try-umount registrations are recorded separately from real mounts.
  Cleanup deletes exactly the recorded registrations after detaching mounts, and
  retains both records on failure so a retry can finish before rebuilding.
- Changes to owned rules/mounts or foreign descendants block cleanup. Baseline
  descendants hidden under an overmount may also force a physical reboot; cleanup
  deliberately does not guess which hidden resources can be detached safely.
- Cleanup is retryable after a handled partial cleanup failure. Fully committed
  ownership with a failed UI snapshot (`syncing`) can also be cleaned up. Interrupted
  application (`applying`) or unverified rollback (`error`) requires a physical
  reboot because some effects may not have been durably identified.
- VFS mutation arms the persistent crash guard. A handled return clears it; process
  termination or a kernel crash leaves it to prevent automatic reinjection. Runtime
  commands never clear a pre-existing guard to force a retry.
- Upgrading during a live session from a version without complete ownership
  (including KernelSU registration records for real mounts) may
  require one physical reboot. Recorded legacy mount targets and untracked explicit
  VFS rules are rejected; old snapshots must not be erased by a no-op cleanup.
- External raw kernel writers do not honor the userspace lock. A check/mutation race
  remains possible; readback and conservative refusal reduce but cannot eliminate
  it. Rule rollback can restore path semantics, not a deleted old source inode.
- No daemon watches module changes. Querying modules merges current installation
  metadata with the runtime snapshot; loading/reloading is explicit. Processes with
  already open file descriptors may retain the old file until they reopen it.

## Validation

Host tests cover ownership reconciliation, rollback failures, foreign changes,
boot/namespace scope, legacy detection, mount-stack cleanup, module eligibility,
reboot parsing and frontend refresh/error handling. Android cross-compilation and
browser mock tests do not prove kernel behavior. Before release, exercise ordinary
and late-load KernelSU on a rooted device, with pure VFS and mixed backends, repeated
soft reboot, retained staging, stock child mounts, replaced/deleted module files,
foreign rule/mount interference, and interrupted apply/cleanup. Check actual file
visibility, `/proc/1/mountinfo`, provider rule readback and `run/runtime.json` together.
