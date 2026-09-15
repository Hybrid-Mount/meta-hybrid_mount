---
name: hm-verify
description: 按 .github/workflows/lints.yml 的门禁在本地跑 Hybrid Mount 全量校验：fmt、clippy -D warnings、测试、禁用符号、LKM 校验、ShellCheck、安装器测试、Android 三架构与 WebUI。
whenToUse: 提交代码、准备 PR、重构或改动挂载/规划逻辑后，需要确认不会挂 CI 时。
---

# Hybrid Mount 本地门禁

与 `.github/workflows/lints.yml` 一一对应。**任何一步失败就停下并报告原始错误**，不要跳步。

## Rust

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## 禁用符号门禁

```bash
set -euo pipefail
if grep -Rni "kasumi" src webui/src module xtask tools 2>/dev/null; then echo "banned symbol kasumi"; exit 1; fi
if grep -REn "normalize_symlinked_partition_layout|normalize_module_layout" src module xtask 2>/dev/null; then echo "banned normalization logic"; exit 1; fi
```

## LKM 来源与校验

```bash
set -euo pipefail
test -f module/lkm/src/LICENSE
test -f module/lkm/src/Kconfig
test -f module/lkm/src/Makefile
test -f module/lkm/README.md
grep -q "GPL-2.0-only" module/lkm/README.md
grep -q "SPDX-License-Identifier: GPL-2.0-only" module/lkm/src/nuke.c
(cd module/lkm/binaries && sha256sum --check list.txt)
```

## 模块脚本

```bash
shellcheck module/*.sh tests/shell/*.sh
sh tests/shell/customize_lkm.sh
```

## Android 交叉检查

CI 使用 nightly 工具链与 CI 容器，三个目标：

```bash
for t in aarch64-linux-android armv7-linux-androideabi x86_64-linux-android; do
  rustup target add "$t" --toolchain nightly
  cargo check -p hybrid-mount --target "$t"
done
```

## WebUI

```bash
cd webui
pnpm install --frozen-lockfile --prefer-offline
pnpm lint    # eslint . && prettier --check .
pnpm test    # vitest run && vue-tsc -b
pnpm build
```

Node 需 >= 24（见 `webui/package.json`）。CI 并行跑这三条，本地串行即可。

## 汇报格式

- 每条命令按 `命令 → 通过/失败` 列出；
- 失败时给出最小复现命令与原始错误片段，不要改代码后再复述。
