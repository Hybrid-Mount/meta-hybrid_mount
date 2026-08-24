# Hybrid Mount 架构

本文描述 v6 `dev` 当前实现。历史重构阶段、orphan 分支操作和已完成的迁移清单不再作为运行契约。

## 维护原则

- `/data/adb/modules/<id>/system/**` 始终是只读输入，安装、扫描、规划和执行阶段都不得移动、合并或删除其中内容。
- v4.2.0 中经过长期实机验证的必要设计可作为回归修复的实现基线；直接移植时必须保留关键语义、许可证和原贡献者归属。
- Magic Mount 行为继续与上游 `meta-magic_mount-rs` 的既定契约核对；混合 planner 负责保证同一路径只进入一个后端。
- 配置字段、CLI 输出和下列稳定路径是 module 脚本与 WebUI 之间的兼容接口，不能只为品牌整理而改名。
- 仓库保留完整 Git 历史和作者信息；完成后的阶段计划可以删除，但不能据此压平或重写贡献记录。

## 启动流水线

```text
module/metamount.sh
  → hybrid-mount（无参数）
  → 读取 config.toml
  → 只读扫描模块与受管分区
  → 生成互斥的 OverlayFS / Magic Mount 计划
  → 预写 scan.ret 与 run/state.json
  → 准备临时 staging
  → 执行 OverlayFS，再执行 Magic Mount
  → 提交 KernelSU try-umount 列表
  → 更新状态快照并清理临时资源
```

流水线不移动、合并或删除 `/data/adb/modules/<id>/system/**`。所有 staging 写入随机临时挂载或 `/data/adb/hybrid-mount` 下的持久资源。若挂载阶段失败，已生成的规划快照仍供 WebUI 显示所选后端和失败状态。

## 代码分层

- `src/config.rs`：TOML schema、默认值、升级兼容与 WebUI patch 持久化。
- `src/scanner.rs`：只读读取模块元数据、状态标记和可挂载条目。
- `src/plan/`：应用“路径规则 > 模块默认值 > 全局默认值”，检测跨后端冲突。
- `src/overlayfs/`：OverlayFS 编排、64 层分段、子挂载重建与文件级 shallow layer。
- `src/magic_mount/`：节点树、分区提升、`.replace`/whiteout 解析与挂载执行。
- `src/storage/`：tmpfs 或 ext4 loop staging；ext4 镜像位于 `/data/adb/hybrid-mount/modules.img`。
- `src/pipeline.rs`：启动顺序、资源生命周期、卸载注册与失败状态持久化。
- `src/state.rs`：`scan.ret`、`run/state.json` 以及 WebUI 所需查询命令。
- `src/sys/`、`src/utils/`：挂载、文件系统、随机临时目录、SELinux xattr 与 KernelSU 接口。
- `webui/`：Vue 3 双界面，通过 `kernelsu.exec` 调用同一个 Rust 二进制。
- `xtask/`：WebUI 构建、Android 交叉编译、module.prop 生成、签名和 ZIP 打包。

## 稳定路径与数据

- 模块目录：`/data/adb/modules/hybrid_mount`
- 二进制：`/data/adb/modules/hybrid_mount/hybrid-mount`
- 配置：`/data/adb/hybrid-mount/config.toml`
- 模块快照：`/data/adb/hybrid-mount/scan.ret`
- 启动状态：`/data/adb/hybrid-mount/run/state.json`
- ext4 staging 镜像：`/data/adb/hybrid-mount/modules.img`

这些路径属于安装、WebUI 和启动脚本之间的兼容接口，不应仅为品牌或目录整理而改名。

发布 ZIP 在安装前包含 `binaries/hybrid-mount-arm64`、`binaries/hybrid-mount-armv7` 和 `binaries/hybrid-mount-x86_64`。`customize.sh` 只复制当前架构对应的文件到上述稳定二进制路径，随后删除安装目录中的 `binaries/`，设备上没有第二套常驻可执行文件。

挂载所需的临时目录使用内核随机生成的 22–30 位字母数字名称和 `0700` 权限，依次尝试 `/tmp`、`/tmp/rw`、`/mnt`。名称不包含项目、PID 或时间戳特征；正常结束时递归清理，仅在 `disable_umount = true` 明确保留挂载时保留对应路径。

## CLI 契约

| 命令 | 用途 |
| --- | --- |
| 无参数 | 执行完整启动挂载流水线 |
| `show-config` | 以 JSON 输出有效配置 |
| `save-config --payload <hex>` | 合并十六进制 UTF-8 JSON patch |
| `gen-config` | 写入默认配置 |
| `modules` | 输出模块与规则快照 |
| `status` | 输出上次启动状态 |
| `install-state` | 输出安装与内核兼容状态 |
| `clear-mount-errors` | 清理模块的 `mount_error` 标记 |
| `emulated-soft-reboot` | 按有效 mount source 懒卸载现有挂载，用于模拟软重启前的清理 |
| `version` | 输出版本 JSON |

WebUI 不持有第二套业务协议：配置与状态请求都映射到以上命令。状态是启动快照，不是 daemon 提供的实时流。

## 验证边界

主机侧可运行 Rust 单元测试、Clippy、WebUI 测试/类型检查和生产构建。Android 三架构编译由 `cargo xtask build` 或 CI 完成。真实 mount、loop、SELinux 与 KernelSU/APatch 交互必须在受支持设备上验证。
