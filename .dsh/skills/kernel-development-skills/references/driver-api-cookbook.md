# Driver API Cookbook

Load this when editing or reviewing Linux kernel driver code. Prefer local
kernel docs and nearby subsystem drivers over memory; vendor and LTS trees may
not have every current upstream helper.

## Table Of Contents

- Source map
- Driver shape
- Resource lifetime
- Embedded API choices
- Subsystem selection
- Runtime PM
- Error handling and logging
- Deprecated and risky patterns
- Version and vendor trees

## Source Map

Check the local kernel tree first:

- `Documentation/driver-api/`
- `Documentation/driver-api/driver-model/`
- `Documentation/driver-api/infrastructure.rst`
- `Documentation/process/deprecated.rst`
- `Documentation/process/coding-style.rst`
- `Documentation/core-api/dma-api-howto.rst`
- `Documentation/driver-api/gpio/`
- `Documentation/power/regulator/`
- `Documentation/driver-api/reset.rst`
- `Documentation/power/runtime_pm.rst`
- nearby drivers in the same subsystem and bus family

Current upstream docs are useful for direction, but target-tree availability
wins. When this skill is used on Linux 6.1 vendor kernels, phrase newer helpers
as "if available in this tree" and check with `rg`.

## Driver Shape

- Use a per-device state container allocated in probe, normally
  `devm_kzalloc(dev, sizeof(*state), GFP_KERNEL)`. Do not use global singleton
  state unless the hardware and subsystem force a singleton and the code
  documents why.
- Store state with the bus helper (`platform_set_drvdata()`,
  `i2c_set_clientdata()`, `spi_set_drvdata()`) before callbacks, IRQs, work, or
  child devices can need it.
- Order probe so resources are acquired before hardware is enabled, internal
  state is initialized before IRQs/work are exposed, and subsystem registration
  happens after the device can answer callbacks.
- Use `dev_err_probe(dev, ret, ...)` for probe failures. It standardizes errno
  logging, returns the error, and records deferred-probe reasons when the error
  is `-EPROBE_DEFER`. Using it is fine even when the error can never be
  `-EPROBE_DEFER`; `dev_warn_probe()`/`dev_info_probe()` exist for graded
  probe logging where available.
- In current mainline, the `remove()` callback returns `void` for platform
  (6.11+, after the `remove_new` transition), i2c (6.1+), and spi (5.18+)
  drivers. Older LTS/vendor trees still use `int remove()` for platform
  drivers; match the signature of the target tree and never "return an error"
  from a remove path to signal failure.
- For purely virtual devices, prefer `faux_device` (6.14+) over fake platform
  devices when the target tree has it; do not abuse the platform bus for
  things that are not platform hardware.
- If probe defers, check `/sys/kernel/debug/devices_deferred` and fix the
  missing supplier, config, or DT dependency. Do not paper over deferral with
  sleeps, retries, or unconditional success.
- Remove/shutdown should stop externally visible activity first: unregister
  from subsystems, disable IRQs or prevent new work, cancel work/timers, quiesce
  hardware, then let devres release memory and simple resources.
- Do not return `-EPROBE_DEFER` after creating child devices unless local driver
  core docs explicitly allow the pattern; it can leave confusing dependency
  state.

## Resource Lifetime

| Need | Prefer | Avoid / check |
| --- | --- | --- |
| Probe-bound memory | `devm_kzalloc()`, `devm_kcalloc()`, `devm_kmalloc_array()` | Open-coded allocation math; use `struct_size()`/`array_size()` helpers for flexible objects. |
| MMIO resource | `devm_platform_ioremap_resource()` or named variant | Raw `ioremap()` without owning/requesting the resource. |
| Custom cleanup | `devm_add_action_or_reset()` when available | Unbalanced partial probe unwinds. |
| Scoped cleanup | `<linux/cleanup.h>`: `guard()`, `scoped_guard()` for locks, `__free()`/`DEFINE_FREE` for allocations and OF node puts, if available in this tree | Mixing scoped cleanup with manual `goto` unwinds for the same resource; backporting the idiom into trees without `cleanup.h`. |
| Subsystem registration | `devm_*_register()` only when callbacks cannot outlive state incorrectly | Blindly managed registration when remove ordering must be explicit. |
| Non-devres lifetime | explicit get/put/refcount pair | Mixing devm and manual release without documenting ordering. |

Devres frees resources on driver detach and failed probe. It does not remove
the need to check return values, disable hardware in the right order, cancel
asynchronous work, or reason about callbacks racing with removal.

## Embedded API Choices

| Surface | Use | Avoid / validate |
| --- | --- | --- |
| MMIO registers | `void __iomem *`, `readb/readw/readl()`, `writeb/writew/writel()`, `ioread*/iowrite*()` when subsystem style uses them | Direct dereference of `__iomem`; missing posted-write readback when hardware requires completion. |
| Register maps | `devm_regmap_init_i2c()`, `devm_regmap_init_spi()`, `devm_regmap_init_mmio()`, `regmap_update_bits()`, `regmap_bulk_read/write()`; `REGCACHE_MAPLE` for new cached regmaps when available (`REGCACHE_RBTREE` is legacy) | Private register caches or bit-twiddling helpers when regmap handles locking, endianness, cache, and variants. |
| Regmap fields | `devm_regmap_field_alloc()` for variant bitfields | Macros hiding different bit layouts across hardware revisions. |
| GPIO consumers | `devm_gpiod_get*()`, logical `gpiod_get/set_value*()`, `gpiod_to_irq()` only when needed | New integer `gpio_*` users; raw GPIO values unless the driver truly needs physical level. |
| Sleeping GPIOs | `gpiod_cansleep()` and `_cansleep` accessors from sleepable context | Accessing I2C/SPI-backed GPIOs in hard IRQ or spinlocked paths. |
| Pinctrl | `devm_pinctrl_get_select_default()`, named states when subsystem needs them | Encoding pinmux policy in driver code when DT/pinctrl should describe it. |
| Regulators | `devm_regulator_get*()`, `regulator_bulk_*()`, balanced enable/disable, `devm_regulator_get_enable*()` if present | Assuming disable removes power from a shared rail; ignoring ramp/startup delays or optional supplies. |
| Clocks | `devm_clk_get*()`, `clk_prepare_enable()`, `clk_disable_unprepare()`, enabled devm helpers if present | `clk_enable()` without prepare; assuming bootloader-enabled clocks remain on. |
| Resets | `devm_reset_control_get_*()`, bulk helpers, shared vs exclusive choice based on line ownership | Deasserting shared resets as if the driver had exclusive hardware control. |
| IRQs | `devm_request_threaded_irq()` when sleeping is needed, IRQ after state init, clear/mask device IRQ source | Sleeping in hard IRQ; requesting IRQ before state can handle callbacks. |
| Work/timers | `INIT_WORK()`, delayed work, hrtimer APIs; cancel before freeing state | Letting work/timers run after unregister/remove; queueing PM work outside `pm_wq` when runtime PM docs require coordination. |
| DMA | `dma_set_mask_and_coherent()`, coherent vs streaming APIs, map/unmap/sync pairs | CPU/device cache assumptions; DMA before mask setup; leaking mappings on error paths. |
| Firmware/config data | `device_property_*()` when firmware-agnostic helps, OF/ACPI helpers when subsystem expects them | Hard-coding board policy in a reusable driver. |
| NVMEM | `devm_nvmem_cell_get()`/read helpers when data is board-provisioned | Reading EEPROMs directly from unrelated drivers when nvmem binding exists. |
| Userspace ABI | subsystem ABI, documented sysfs in `Documentation/ABI/`, ioctl/netlink only when subsystem expects it | Debugfs or module params as stable product ABI. |

Regmap is not mandatory for every small driver. Use it when it removes real
bus/register complexity, provides locking/cache/endianness value, or makes
hardware variants clearer.

## Subsystem Selection

Before writing a private interface, ask whether the device already belongs in a
kernel subsystem:

| Device behavior | Usually belongs in |
| --- | --- |
| voltage/current control | regulator |
| sensors/ADCs/DACs | IIO or hwmon depending on semantics |
| thermal readings/cooling | thermal, hwmon for observability |
| buttons/keys/touch | input |
| batteries/chargers/USB power | power_supply, USB Type-C, extcon only when appropriate |
| LEDs/backlights | leds, backlight |
| media bridges/cameras/panels | V4L2/media, DRM bridge/panel |
| persistent calibration/identity | nvmem |
| clocks/resets/pinctrl/GPIO providers | clk, reset, pinctrl, gpiolib |

If a subsystem exists, follow its registration, locking, PM, and ABI model even
when a private char device would be faster to write.

## Runtime PM

- Add runtime PM when the hardware has meaningful idle states, shared suppliers,
  or subsystem expectations. Avoid PM boilerplate that cannot be tested.
- Use `struct dev_pm_ops` and subsystem helpers/macros. Keep runtime PM and
  system sleep callbacks consistent. In current trees prefer
  `DEFINE_RUNTIME_DEV_PM_OPS()`/`DEFINE_SIMPLE_DEV_PM_OPS()` with `pm_ptr()`
  (or `RUNTIME_PM_OPS`/`SYSTEM_SLEEP_PM_OPS` and `pm_sleep_ptr()`) instead of
  `SIMPLE_DEV_PM_OPS` plus `#ifdef CONFIG_PM` blocks.
- Use `devm_pm_runtime_enable()` when available so error paths cannot leave
  runtime PM enabled/disabled unbalanced.
- Prefer `pm_runtime_resume_and_get()` over `pm_runtime_get_sync()` when the
  target tree provides it and the caller checks errors.
- Balance every successful get with put/autosuspend. On errors after a get,
  unwind before returning.
- Serialize runtime suspend/resume against IRQs, work, register access, and
  subsystem callbacks. A suspended device must not receive unguarded MMIO/bus
  accesses.
- Validate with runtime suspend/resume evidence, not only boot probe logs.

## Error Handling And Logging

- Return kernel errno values that match the failure. Preserve supplier errors
  when they carry useful meaning, especially `-EPROBE_DEFER`.
- Use `dev_err_probe()` in probe. Use `dev_err()` for real runtime failures,
  `dev_warn()` for degraded operation, and `dev_dbg()` for diagnostics.
- Prefer dynamic debug, tracepoints, ftrace, or subsystem debugfs for temporary
  observability. Do not leave noisy `dev_info()` or `pr_info()` traces in hot
  paths.
- `WARN*()` is for conditions expected to be unreachable. Do not use it for
  ordinary invalid input, missing hardware, or recoverable runtime failures.
- Never use `BUG()`/`BUG_ON()` in driver code for validation or convenience.

## Deprecated And Risky Patterns

| Avoid | Prefer |
| --- | --- |
| `BUG()`, `BUG_ON()` | return errors, recover, or use `WARN_ON_ONCE()` only for unreachable states |
| `simple_strto*()` | `kstrto*()` |
| `strcpy()` | `strscpy()` or `strscpy_pad()` |
| `strncpy()` for C strings | `strscpy()`/`strscpy_pad()`; `strtomem()` for non-NUL fixed fields |
| `strlcpy()` (removed from the kernel in v6.8) | `strscpy()` |
| `kmalloc(count * size, ...)` | `kmalloc_array()`, `kcalloc()`, `array_size()` |
| struct plus trailing array math | flexible array member plus `struct_size()` |
| zero-length or one-element trailing arrays | C99 flexible array members |
| raw `%p` logging | remove it, use symbolic formats such as `%pS`, or justify privileged `%px` |
| VLAs on stack | fixed-size bounds or heap allocation |
| new integer `gpio_*` consumer APIs | descriptor GPIO APIs |
| ad hoc sysfs files | documented subsystem ABI or `Documentation/ABI/` entry |

## Version And Vendor Trees

- Check helper availability with `rg` before prescribing a current upstream
  helper in an older LTS/vendor tree.
- Prefer local subsystem patterns over newer examples if the subsystem has not
  adopted the helper in this tree.
- When backporting, preserve upstream behavior and metadata where possible, but
  validate against local DT, firmware, rootfs packages, and out-of-tree users.
