# Kernel Documentation Routes

Curated entry points into a kernel checkout's `Documentation/` tree. Paths
track current mainline; vendor and LTS trees can differ (for example, arch
docs moved under `Documentation/arch/` around v6.5), so verify against the
checkout you are working in. For exhaustive lookups, generate the
per-checkout index described below instead of trusting memory.

## Curated Routes

### process

- `Documentation/process/submitting-patches.rst`
- `Documentation/process/submit-checklist.rst`
- `Documentation/process/coding-style.rst`
- `Documentation/process/handling-regressions.rst`
- `Documentation/process/stable-kernel-rules.rst`
- `Documentation/process/maintainer-handbooks.rst`
- `Documentation/maintainer/index.rst`

### debug-tests

- `Documentation/dev-tools/testing-overview.rst`
- `Documentation/dev-tools/kunit/index.rst`
- `Documentation/dev-tools/kselftest.rst`
- `Documentation/dev-tools/ktap.rst`
- `Documentation/admin-guide/dynamic-debug-howto.rst`
- `Documentation/admin-guide/ramoops.rst`
- `Documentation/admin-guide/kdump/index.rst`
- `Documentation/dev-tools/kgdb.rst`
- `Documentation/dev-tools/gdb-kernel-debugging.rst`

### tracing

- `Documentation/trace/ftrace.rst`
- `Documentation/trace/events.rst`
- `Documentation/trace/kprobes.rst`
- `Documentation/trace/fprobe.rst`
- `Documentation/trace/histogram.rst`

### sanitizers-static-analysis

- `Documentation/dev-tools/checkpatch.rst`
- `Documentation/dev-tools/sparse.rst`
- `Documentation/dev-tools/coccinelle.rst`
- `Documentation/dev-tools/kasan.rst`
- `Documentation/dev-tools/kmsan.rst`
- `Documentation/dev-tools/kcsan.rst`
- `Documentation/dev-tools/kfence.rst`
- `Documentation/dev-tools/kmemleak.rst`
- `Documentation/fault-injection/fault-injection.rst`

### devicetree-drivers

- `Documentation/devicetree/index.rst`
- `Documentation/devicetree/bindings/writing-schema.rst`
- `Documentation/devicetree/bindings/submitting-patches.rst`
- `Documentation/devicetree/bindings/dts-coding-style.rst`
- `Documentation/driver-api/index.rst`
- `Documentation/driver-api/driver-model/index.rst`
- `Documentation/driver-api/regulator.rst`
- `Documentation/driver-api/gpio/index.rst`
- `Documentation/driver-api/pin-control.rst`
- `Documentation/driver-api/i2c.rst`
- `Documentation/driver-api/spi.rst`
- `Documentation/driver-api/clk.rst`
- `Documentation/driver-api/thermal/index.rst`

### kbuild-abi

- `Documentation/kbuild/index.rst`
- `Documentation/kbuild/kconfig-language.rst`
- `Documentation/kbuild/kconfig.rst`
- `Documentation/kbuild/makefiles.rst`
- `Documentation/admin-guide/abi.rst`
- `Documentation/ABI/README`
- `Documentation/userspace-api/index.rst`

## Per-Checkout Generated Index

The route map above is curated and stable. For keyword routes and a full
machine-readable index of the exact tree you are working in, generate the
per-checkout artifacts under `assets/` (never commit them; they embed
tree-specific content):

```bash
python3 scripts/index_kernel_docs.py \
  /path/to/kernel-root \
  assets/kernel-doc-routes.md \
  --jsonl assets/kernel-doc-index.jsonl
```

Run from this skill's directory; both outputs land in this skill's `assets/`.

## Full Index Usage

Do not load the full JSONL into context. Query it with `rg`, `jq`, or a short
script, then open only the relevant kernel docs.

Examples:

```bash
rg '"keywords":.*kselftest' assets/kernel-doc-index.jsonl
rg 'rk3576|lapis|pwm-fan' assets/kernel-doc-index.jsonl
```
