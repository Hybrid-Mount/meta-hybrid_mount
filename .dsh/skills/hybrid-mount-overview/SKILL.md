---
name: hybrid-mount-overview
description: Hybrid Mount 元模块的项目导航与硬性约束：启动流水线、规则优先级、共享挂载树不变量、目录地图与禁止操作。改代码前先读。
whenToUse: 任务涉及 Hybrid Mount 架构、模块扫描/规划/挂载流程，或需要确认目录职责与硬性约束时。
---

# Hybrid Mount 项目导航

面向 KernelSU / APatch 的混合挂载元模块。Rust（edition 2024）+ Vue 3 WebUI，
仓库根即 Cargo workspace 根，主 crate 名为 `hybrid-mount`。

## 启动流水线（`src/pipeline.rs`，无参数入口）

1. **config** 读取 `/data/adb/hybrid-mount/config.toml`
2. **scan** 只读扫描 `/data/adb/modules`，构建 `ModuleRecord`
3. **plan** 构建 `MountPlan`（统一节点树 + 分后端操作列表）
4. **storage** 准备 overlay staging（tmpfs / ext4 loop）
5. **overlay** 执行 OverlayFS（直接目录层 + shallow 文件层）
6. **magic** 执行 Magic Mount（bind mount + symlink + whiteout）
7. **commit** 提交 KSU unmount 列表，写状态快照

任一挂载阶段失败 → 事务式回滚，并保留失败状态快照供 WebUI 查询。

## 规则优先级（高 → 低）

```
路径规则 > 模块 default_mode > 全局 default_mode
```

## 不可违反的不变量

- 模块源目录是**只读输入**，任何 staging 写入都落在运行目录。
- 同一文件路径只能进入一个后端；普通目录可被两个后端共享为结构节点。
- 文件 / 类型 / `.replace` 冲突必须在 **plan 阶段显式报错**，executor 只执行、不裁决。
- 回滚必须清理所有挂载点与临时目录。

## 目录地图

| 路径 | 职责 |
| --- | --- |
| `src/pipeline.rs` | 启动流水线与回滚事务 |
| `src/config.rs` | TOML 解析与校验 |
| `src/scanner.rs` | 只读模块扫描 |
| `src/plan/mod.rs` | 规划器（统一节点树） |
| `src/mount_tree.rs` | 跨后端共享节点树 |
| `src/overlayfs/` · `src/magic_mount/` | 两个执行后端 |
| `src/storage/` | overlay staging（tmpfs / ext4） |
| `src/sys/` | 文件系统 / 挂载 / nuke 辅助层 |
| `src/state.rs` | 运行状态快照（WebUI 读取） |
| `src/cli.rs` | 手工 CLI 参数解析与分派 |
| `module/` | 安装与启动脚本、LKM、默认 config.toml |
| `webui/` | Vue 3 + Vite WebUI |
| `docs/` | 多语言文档 + ARCHITECTURE.md |
| `xtask/` | 构建 / 发布自动化 |

## CLI 子命令（`src/cli.rs`）

无参数 = 执行挂载流水线。其余：`show-config`、`save-config`、`gen-config`、
`modules`、`status`、`version`、`install-state`、`clear-mount-errors`、`emulated-soft-reboot`。
面向 WebUI 的 JSON 只走标准输出，诊断日志走 `log` crate，两者不要混用。

## 禁止事项

- 禁用符号（workspace lint 直接 `deny`）：dbg!、expect()、todo!、unimplemented!、unwrap()。
  错误处理用 `Result` + `?`，自定义错误见 `src/errors.rs`。
- 禁止符号`kasumi`（已移除的旧逻辑）。
- 禁止 `normalize_symlinked_partition_layout` / `normalize_module_layout`（已移除的布局规范化）。
- 路径操作一律 `Path`/`PathBuf`，不要字符串拼接。

配套 skill：hm-verify、hm-build-release、hm-planner-debug、hm-mount-safety-review、hm-webui。
