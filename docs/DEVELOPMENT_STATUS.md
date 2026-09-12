# 开发收尾记录（2026-09-11）

基线为 `244512a6`，本轮基础修复与 Issue #409 的修复已分批提交并推送到 `dev`。两个依赖 PR 的全部检查通过后，已于 2026-09-11 分别 squash 合并到 dev。未发布正式版本。

## 分批提交

| 提交 | 内容 |
| --- | --- |
| `39597c27` | 原子写撞名重试、残留清理、LKM guard 持久化、错误标记与缓存一致性 |
| `f2ceb6a6` | 删除后端冗余辅助代码 |
| `cbb1c8e6` | WebUI 类型契约、Miuix 组件和规范检查修复 |
| `1860f0e0` | 构建文档、维护脚本与开发说明 |
| `1ba20add` | #409：兼容旧配置中的 daemon_startup_mode，保留挂载设置和规则 |
| `a7fcbfd8` | #410（PR 内修复提交 `78bb317c`）：迁移 tgbot 0.48 文件上传与 HTML caption API，增加请求构造回归测试 |
| `a4929b1f` | #411（PR 内格式提交 `34041802`）：规范化升级后的依赖锁文件格式 |

## Issue #409

已读取报告者提供的 bugreport。启动日志确认配置阶段失败：旧配置中的 `daemon_startup_mode = "persistent"` 被视为未知字段，导致在扫描模块前退出。

修复只兼容这个废弃字符串字段，不放宽其他未知字段、错误类型或损坏配置的检查。加载不覆盖原文件，后续保存会去掉旧字段；回归测试覆盖保留全局配置、模块和路径规则、兼容读取与保存，以及错误配置继续失败。

已在 [Issue 回复](https://github.com/Hybrid-Mount/meta-hybrid_mount/issues/409#issuecomment-5617324810) 说明原因、修复和临时处理方式，并提供 [dev 构建 1924](https://github.com/Hybrid-Mount/meta-hybrid_mount/actions/runs/34467055476)。Issue 保持开放，等待报告者真机重启确认。原始 bugreport 没有加入仓库。

## PR 处理（均已合并）

- [#410](https://github.com/Hybrid-Mount/meta-hybrid_mount/pull/410)：将已移除的 document 工厂方法改为 From/Into；caption 与 HTML 格式使用新版输入类型。测试构造单文件与媒体组请求，验证 HTML、话题 ID、文件类型和组内 caption，没有调用 Telegram。
- [#411](https://github.com/Hybrid-Mount/meta-hybrid_mount/pull/411)：合入基础修复后，安装 7 项升级依赖，修正锁文件格式；lint、69 个测试、类型检查和生产构建全部通过。

两个 PR 的构建、Linux 测试、WebUI 检查、三个 Android 架构检查及依赖审计全部成功；#410 的 Notify 独立检查也通过。

## 验证

- #409 修复在本机通过 Cargo workspace 测试（218 核心、4 notify、2 xtask），并通过严格 Clippy、三个 Android 架构编译检查。
- #410 与 #409 合并后的组合在本机通过 225 个 workspace 测试（218 核心、5 notify、2 xtask）及严格 Clippy。
- WebUI 的旧依赖和 #411 新依赖组合均通过 69 个测试、类型检查、lint 和生产构建。
- 安装脚本测试和 ShellCheck 通过。
- `1ba20add` 的远端 [lints-check](https://github.com/Hybrid-Mount/meta-hybrid_mount/actions/runs/34467055368) 与 [完整打包](https://github.com/Hybrid-Mount/meta-hybrid_mount/actions/runs/34467055476) 均成功，覆盖 Linux 测试及三个 Android 架构。

未在报告者设备上运行；Android 实际挂载、LKM 加载和重启恢复仍需真机验证。

## 尚未纳入本轮的后端审查条目

HM-RUST-009 仍未闭环：mountsource 校验只在启动入口调用，软重启入口未调用，按 source 卸载也未限制为本项目目标。其余遗留审查条目见 `RUST_BACKEND_REVIEW.md`；本轮没有重新执行完整安全审查。

## 2026-09-12：Telegram 单文件上传回归

PR 检查不执行真实 Telegram 通知。合并后的构建虽然完成打包，却在通知阶段收到 `400 unsupported parse_mode`。

本地 HTTP 接收测试复现了 tgbot 0.48 的 multipart 编码问题：单文件上传将 `parse_mode` 编码成带 JSON 引号的 `"HTML"`，而 Telegram 要求该表单字段为 `HTML`。媒体组的嵌套 JSON 格式正常。上一轮基于 Debug 文本的请求测试无法区分这两种编码，因此没有捕获此问题。

暂将 tgbot 精确固定为 0.46.0，并恢复对应的 caption / document 构造 API。用真实 HTTP 请求测试替换 Debug 文本测试，覆盖单文件的原始 parse_mode、完整 caption、chat_id、话题 ID、文件内容，以及媒体组中的 caption 和格式字段。测试仅访问本机接收端，使用假 token；不读取 secrets，也不发送 Telegram 消息。待上游版本通过这些测试后再升级。
