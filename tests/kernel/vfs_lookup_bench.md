# Child lookup measurement (2026-10-17)

`vfs_lookup_bench.py` extracts `hybridmount_bsearch_child` verbatim from the
current production source and compiles it into a native C harness. Run from any
directory with Python 3 and a GCC/Clang-compatible C compiler:

```sh
CC=cc python3 tests/kernel/vfs_lookup_bench.py
```

On this Windows workstation WSL was unavailable. The existing Zig compiler was
used instead; in PowerShell:

```powershell
$env:CC='C:/tools/zig/zig.exe cc'
python tests/kernel/vfs_lookup_bench.py
```

The harness prints source-function SHA256, compiler invocation and CSV. Generated
headers and executables live in a temporary directory. Assertions stay enabled
even when the compiler defines `NDEBUG`. It checks all measured results against
an independent linear name oracle and separately checks full-hash collision
resolution and missing-name/missing-hash cases.

## Scope

This is a **userspace, hot-cache microbenchmark**, not kernel VFS performance.
It models HM's fixed64 gate and tagged-single behavior, compares the extracted
production search with upstream-sized adaptive filters, and separately models
direct matching for at most two children. Hashes are seeded synthetic 32-bit
values, associated consistently with names; hashing itself is excluded. Actual
`full_name_hash` distributions, credentials, UID filtering, RCU/SRCU, seqcount
retry, concurrent updates, allocation, cache pressure and real filesystem work
are excluded. No ARM64 or Android measurements were made.

Each mode uses 65,536 queries, 32 repetitions, and the median of five samples;
sample mode order rotates. Positive queries select existing names, negative
queries use absent names. A colliding full hash is still resolved by exact name.
Both workloads and counts 1, 2, 16, 64 and 256 run every time.

## Observed results

Machine: Windows x86_64, Intel Core Ultra 7 270K Plus, Zig 0.14.1 `cc -O2`.
Extracted function SHA256:
`e3dadd47b13c987880ae7a55d14ccb07273c9c9b688a992b20a7e5d99c124431`.
Two complete runs passed. Timings below span the two run medians; deterministic
gate pass rates are identical in both. Other processes were active, so small
timing differences should not determine policy.

| Children | Fixed64 negative gate pass | Adaptive negative gate pass | Fixed64 negative ns/query | Adaptive negative ns/query | Filter payload, fixed/adaptive |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1.4954% | 0.3906% | 1.379–2.030 | 1.308–2.076 | 8 / 32 B |
| 2 | 3.0334% | 0.7370% | 1.468–2.313 | 1.418–1.798 | 8 / 32 B |
| 16 | 25.0885% | 6.2119% | 5.404–6.612 | 2.521–2.894 | 8 / 32 B |
| 64 | 67.1326% | 6.3599% | 15.006–15.780 | 2.857–3.076 | 8 / 128 B |
| 256 | 96.7667% | 6.1493% | 24.203–25.815 | 3.567–3.633 | 8 / 512 B |

Adaptive sizes follow upstream `a362f608`: 4 words through 16 children, 16 words
through 64, and 64 words through 256. Logical filter payload excludes metadata,
alignment, slab classes and retired snapshots. If HM retains its existing
directory-node fixed64 mask as an outer gate, the adaptive payload is **additional**
memory, not a replacement. Existing child arrays have separate capacity overhead;
their exact allocation size is not measured by this harness.

Positive queries pass both filters 100% of the time, so an adaptive filter does
not remove binary-search work. At 16/64/256 children the observed positive times
were respectively about 20/25–26/32–33 ns/query, with no consistent adaptive
advantage. For one child direct matching was slower than fixed64 in both positive
and negative tests; for two children it improved positive queries but was not a
consistent negative-workload improvement. This model does not justify removing
HM's existing tagged-single fast path or its small-directory Bloom gate.

## Adaptation decision

The measurements justify an adaptive-filter **candidate for larger,
negative-heavy directories**. They do not establish a kernel end-to-end win or
justify blindly transplanting upstream topology. Production adoption should wait
for representative on-device directory-size/miss measurements and concurrent
update verification. Retaining fixed64 now is a defensible result of the report's
"measure first" recommendation.

If implemented later, the smallest compatible design is:

1. Preserve tagged single children and the existing 64-bit outer gate. Allocate
   adaptive storage only with multi-child snapshots selected by capacity.
2. Store adaptive word count and words together with the published child-array
   allocation. Select size by capacity so ordinary in-place insertion cannot
   cross a size threshold. Rebuild the complete filter on replacement/resizing.
3. Read/test the adaptive filter **inside the same seqcount retry region and
   read-side lifetime protection as the child search**. A changed sequence must
   retry even when the filter says "absent". The outer lookup/actor gate is
   currently outside that retry region; it cannot safely use separately
   published adaptive pointer/size state. Keep outer fixed64 updates consistent
   with the existing children publication protocol.
4. Use current SRCU synchronization and RCU reclamation for replaced arrays.
   Array storage is also consumed by directory emission across `dir_emit`.
   Do not replace this with upstream's different reclamation model.
5. Exercise growth/shrink thresholds, replacement at unchanged count, equal
   full hashes, delete-to-single, clear, allocation failure and concurrent
   lookup/readdir before adoption. Profile hashing and misses on device.

## Reusing fsuid within one directory iteration

This is independent of Bloom sizing. Current code reads fsuid once at iterate
entry for the blocklist and again in `__hybridmount_get_rule_info` for each
Bloom-positive actor query. A safe minimal refactor passes an explicit fsuid to
the internal resolver and stores the entry snapshot in each proxy context; normal
lookup wrappers continue obtaining their own current fsuid. Initialize **both**
hijacked and virtual-directory proxy constructors. If the emission helper also
accepts the snapshot, use the same uid for target-UID visibility throughout that
iteration. Never persist it in directory nodes or across separate iterate calls;
credentials may change between calls. Preserve existing blocklist and target-UID
semantics.

The source makes the eliminated per-entry credential reads clear, but their
actual cost is not measured here. A userspace volatile read would not represent
kernel `current_fsuid()` and would give a misleading speedup claim. Benchmark
target-UID/blocklist semantics separately, including different credentials on
successive calls; make no claimed credential-cache performance improvement.
