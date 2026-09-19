# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Hybrid Mount 是面向 KernelSU 与 APatch 的混合挂载元模块。它在启动阶段扫描其他模块，按全局、模块和路径规则为每一项选择 OverlayFS、Magic Mount 或忽略，并且始终把模块源目录当作只读输入。

- **核心语言**: Rust (edition 2024)
- **目标平台**: Android (aarch64, armv7, x86_64)
- **WebUI**: Vue 3 + TypeScript + Vite
- **构建系统**: cargo + xtask

## Common Commands

### Rust 开发

```bash
# 完整构建（WebUI + 二进制 + module zip）
cargo xtask build

# 发布构建
cargo xtask build --release

# 运行测试
cargo test --workspace

# 格式化
cargo fmt --all

# Clippy 检查
cargo clippy --workspace --all-targets -- -D warnings

# 交叉编译检查（针对 Android 目标）
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo check -p hybrid-mount --target aarch64-linux-android
```

### WebUI 开发

```bash
cd webui

# 安装依赖
pnpm install --frozen-lockfile

# 开发服务器
pnpm dev

# 构建
pnpm build

# Lint
pnpm lint

# 单元测试与类型检查
pnpm test

# 仅类型检查
pnpm typecheck
```

### 模块脚本检查

```bash
# ShellCheck
shellcheck module/*.sh tests/shell/*.sh
```

## Architecture

### 核心流水线

启动流水线（`src/pipeline.rs`）是无参数入口，执行顺序：

1. **config** - 读取 `/data/adb/hybrid-mount/config.toml`
2. **scan** - 只读扫描 `/data/adb/modules`，构建 `ModuleRecord` 列表
3. **plan** - 根据规则构建 `MountPlan`（统一节点树 + 分后端操作列表）
4. **storage** - 准备 overlay staging（tmpfs 或 ext4）
5. **overlay** - 执行 OverlayFS 挂载（直接目录层 + shallow 文件层）
6. **magic** - 执行 Magic Mount（bind mount + symlink + whiteout）
7. **commit** - 提交 KSU unmount 列表，写状态快照

挂载阶段失败时触发事务式回滚，并保留失败状态快照供 WebUI 查询。

### 规则优先级

```
路径规则 > 模块 default_mode > 全局 default_mode
```

同一文件路径只能进入一个后端；普通目录可由两个后端共享作为结构节点；文件、类型与 `.replace` 冲突在启动规划阶段显式报错。

### 模块结构

- `src/config.rs` - TOML 配置解析与验证
- `src/scanner.rs` - 只读模块扫描，不修改源目录
- `src/plan/mod.rs` - 混合挂载规划器，构建统一节点树
- `src/mount_tree.rs` - 跨后端共享的节点树结构
- `src/pipeline.rs` - 启动流水线与回滚事务
- `src/overlayfs/` - OverlayFS 挂载逻辑
- `src/magic_mount/` - Magic Mount 执行逻辑
- `src/storage/` - overlay staging 存储（tmpfs / ext4）
- `src/sys/` - 系统辅助层（文件系统、挂载、nuke）
- `src/state.rs` - 运行状态快照（供 WebUI 读取）

### WebUI 通信

WebUI 通过管理器提供的 `kernelsu.exec` 桥接调用同一个 Rust 二进制，使用 `show-config`、`save-config`、`modules`、`status` 等命令交换 JSON。后端管理以下持久化文件：

- `/data/adb/hybrid-mount/run/state.json` - 运行状态快照
- `/data/adb/hybrid-mount/scan.ret` - 模块列表快照
- `/data/adb/hybrid-mount/config.toml` - 当前配置

写入配置后需重启生效。

### 分区提升

支持动态分区提升（vendor、product、odm 等）。planner 检测 `/system/vendor` 等是否为符号链接，决定是否提升到顶层分区根。提升后 `system/vendor/lib` 映射到 `/vendor/lib`。

### 存储模式

- **tmpfs** - 内存中 staging，速度快但占用 RAM
- **ext4** - loop 设备 + ext4 文件系统，持久化 staging

ext4 模式在 KernelSU 使用官方 `NukeExt4Sysfs` ioctl 隐藏 sysfs 节点；在 APatch 等非 KSU 环境使用随附 LKM（仅 aarch64）。LKM 加载期间若内核崩溃，遗留的持久熔断标记会阻止下次启动再次尝试。

## Code Conventions

### Rust 代码规范

- **禁用符号**: `dbg!`, `expect()`, `todo!()`, `unimplemented!()`, `unwrap()`（workspace lint）
- **错误处理**: 使用 `Result<T>` 和 `?` 操作符，自定义 `Error` 类型在 `src/errors.rs`
- **日志**: 使用 `log` crate（`log::info!`, `log::error!` 等）；CLI 的 JSON 输出使用标准输出，不混入诊断日志
- **平台特定代码**: 用 `#[cfg(any(target_os = "linux", target_os = "android"))]` 隔离
- **测试**: 单元测试在模块末尾 `#[cfg(test)] mod tests`，集成测试在项目根目录 `tests/` 下

### 代码审查要点

- 挂载与文件操作必须检查返回值
- 路径操作使用 `Path` / `PathBuf`，避免字符串拼接
- 回滚逻辑必须清理所有资源（挂载点、临时目录）
- 新增配置字段需同步更新 `config.toml` 和文档
- 跨模块冲突检测在 planner 阶段完成，executor 只执行

### 禁止的操作

- **禁止修改模块源目录** - 所有 staging 写入运行目录
- **禁止符号 "kasumi"** - 已移除的旧逻辑
- **禁止 `normalize_symlinked_partition_layout`** - 已移除的布局规范化

## Testing

```bash
# 运行所有测试
cargo test --workspace

# 运行特定模块测试
cargo test -p hybrid-mount

# 运行特定测试
cargo test test_name

# 显示测试输出
cargo test -- --nocapture
```

测试覆盖：
- planner 规则优先级与冲突检测
- overlay 路径映射与 staging
- magic mount 执行逻辑
- 配置解析与验证
- 回滚事务正确性

## Platform-Specific Notes

### Android 目标

- 使用 Android NDK 交叉编译
- 目标架构: `aarch64-linux-android`, `armv7-linux-androideabi`, `x86_64-linux-android`
- `xtask` 的 Android 构建指定最低 API level 26
- 运行时需 root 权限（KernelSU 或 APatch）

### LKM（可选）

随包提供两个独立的内核模块，均由 CI 在 DDK 镜像中编译，产物连同 SHA256 清单提交回仓库：

- **ext4 sysfs nuke LKM**
  - 位置: `module/lkm/`
  - 许可证: GPL-2.0-only
  - 仅 aarch64 预编译，来源: Mountify 项目
  - 校验: `module/lkm/binaries/list.txt` 包含 SHA256
- **VFS 内核子系统（`hybridmount`）**
  - 位置: `module/vfs/`（源码在 `module/vfs/src/`）
  - 许可证: GPL-2.0-only，fork 自 NoMount
  - 仅 aarch64 预编译，按 Android/GKI 目标命名 `hybridmount-android<NN>-<kernel>.ko`
  - 校验: `module/vfs/binaries/list.txt` 包含 SHA256（由 `.github/workflows/kernel-module.yml` 生成）
  - 防漂移: `module/vfs/binaries/sources.txt` 记录构建输入（`hybridmount.c`、`hybridmount.h`、`Kconfig`、`Makefile`）的摘要，由真正编译它们的 job 写入；`tests/shell/vfs_sources_digest.sh` 重算比对，`lints.yml` 与 `release.yml` 都会执行。`kernel-module.yml` 在推送到 `dev` 且改动 `module/vfs/**` 时自动重建并提交产物，因此源码变更与产物刷新是两个提交，中间的推送会在该门禁上失败属预期

启动流程先探测内建的 key type `hybridmount`；未响应且配置中确有规则选择 `vfs` 时，从 `module/vfs/binaries/` 选择与内核线精确匹配的模块加载后再探测。加载必须发生在规划之前：planner 会把无后端可用的 `vfs` 规则降级为 `ignore`，执行器随之提前返回而走不到加载分支，等到执行阶段才加载会永远加载不上。`vfs_strict` 的失败判定同样在规划前完成，且仅当确有规则选择 `vfs` 时生效。

### 熔断机制

两个模块共用 `src/sys/lkm.rs` 的熔断逻辑：在调用 `insmod` 前持久化熔断标记，加载尝试正常返回时移除标记。若内核在加载期间崩溃，标记会保留，下次启动跳过对应模块但保留 Hybrid Mount 其他功能。

## Important Files

- `Cargo.toml` - workspace 配置与版本
- `xtask/src/main.rs` - 构建自动化脚本
- `module/` - KernelSU/APatch 模块脚本
- `module/lkm/` - ext4 sysfs nuke LKM
- `webui/` - Vue 3 WebUI 源码
- `docs/` - 多语言文档
- `.github/workflows/` - CI/CD 配置

## Release Process

`cargo xtask build --release` 完成：

1. 安装 WebUI 依赖并构建（`pnpm build`），输出到 `module/webroot`
2. 通过 Vite 将 `MODULE_ID` 注入 WebUI
3. 交叉编译 Rust 二进制（aarch64、armv7、x86_64）
4. 生成 `module.prop`
5. 打包 zip（包含二进制、脚本、WebUI、LKM）

`.github/workflows/release.yml` 负责发布与更新 `update.json`、`changelog.md` 和版本信息；`cargo xtask update-json` 也可单独生成更新元数据。Telegram 通知由独立的 `cargo xtask notify` 命令发送，构建命令不会自动发送。

### Tag 约定

发布 tag 必须是合法 semver，**patch 不允许前导零**：`v6.2.1` 可以，`v6.2.01` 不行（Cargo 会拒绝 `version = "6.2.01"`）。预发布写成 `-<stage>.<number>`，stage 取 `alpha`、`beta`、`rc`，number 为 1–99：

```bash
cargo xtask release-version v6.2.1-rc.1   # 输出 version / version_code / prerelease
```

tag 到版本、versionCode 与预发布标志的换算只由 `xtask` 的 `ReleaseVersion` 决定，workflow 不再自带正则或 awk，避免出现「workflow 接受的 tag 被 Cargo 拒绝」这类只在构建阶段暴露的偏差。

versionCode 为 `major*100000 + minor*1000 + patch` 再乘 1000 加槽位；预发布槽位**低于**对应正式版（`v6.2.1-rc.1` = 602001301 < `v6.2.1` = 602001999），否则试过预发布的设备永远收不到正式版。新 code 一律高于历史上按整数发布的 code（如 `v6.2.0` = 602000），升级链路不受影响。

## Git Workflow

- 主分支: `main`
- 开发分支: `dev`
- PR 目标: `dev`
- 发布时从 `dev` 合并到 `main`

## Contact

- GitHub Issues: https://github.com/Hybrid-Mount/meta-hybrid_mount/issues
- Telegram: @hybridmountchat
