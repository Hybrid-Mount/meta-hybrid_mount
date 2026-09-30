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
3. **vfs probe** 探测 keyring 中 `hybridmount` 键类型；静默时加载随包模块
4. **plan** 构建 `MountPlan`（统一节点树 + 分后端操作列表）
5. **storage** 准备 overlay staging（tmpfs / ext4 loop）
6. **overlay** 执行 OverlayFS（直接目录层 + shallow 文件层）
7. **magic** 执行 Magic Mount（bind mount + symlink + whiteout）
8. **vfs** 提交 VFS 规则（keyring），失败按 `vfs_strict` 决定降级或终止
9. **commit** 提交 KSU unmount 列表，写状态快照

第 3 步必须在第 4 步之前：provider 静默时规划器会把 `vfs` 规则改写成 `ignore`，
executor 随后会直接提前返回。后端 mode 取值为 `overlay | magic | vfs | ignore`；
`vfs` 只有在探测或加载成功时才生效，否则规划阶段降级为 `ignore`。

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
| `src/sys/` | 文件系统 / 挂载 / nuke / LKM 加载器 |
| `src/vfs/` | VFS 后端控制面：规则映射、wire protocol、keyring 通信、LKM 选型与加载 |
| `src/runtime/` | 所有权账本、规则事务、挂载身份校验（见 `docs/RUNTIME.md`） |
| `src/state.rs` | 运行状态快照（WebUI 读取） |
| `src/cli.rs` | 手工 CLI 参数解析与分派 |
| `module/` | 安装与启动脚本、LKM、默认 config.toml |
| `webui/` | Vue 3 + Vite WebUI |
| `docs/` | 多语言文档 + ARCHITECTURE.md |
| `xtask/` | 构建 / 发布自动化 |

## CLI 子命令（`src/cli.rs`）

无参数 = 执行挂载流水线，`boot` = 带启动锁的流水线入口。其余：`show-config`、
`save-config`、`gen-config`、`modules`、`status`、`version`、`install-state`、
`clear-mount-errors`、`vfs-doctor`、`runtime`、`vfs`、`lkm-load`、`emulated-soft-reboot`。
`runtime`、`vfs`、`lkm-load` 都把剩余参数交给各自的 `handle(&args[1..])`。
面向 WebUI 的 JSON 只走标准输出，诊断日志走 `log` crate，两者不要混用。

## 禁止事项

- 禁用符号（workspace lint 直接 `deny`）：dbg!、expect()、todo!、unimplemented!、unwrap()。
  错误处理用 `Result` + `?`，自定义错误见 `src/errors.rs`。
- 禁止符号`kasumi`（已移除的旧逻辑）。
- 禁止 `normalize_symlinked_partition_layout` / `normalize_module_layout`（已移除的布局规范化）。
- 路径操作一律 `Path`/`PathBuf`，不要字符串拼接。

配套 skill：hm-verify、hm-build-release、hm-planner-debug、hm-mount-safety-review、hm-webui。
