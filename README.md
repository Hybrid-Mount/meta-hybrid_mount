# ReHybrid-Mount

ReHybrid-Mount 是 Hybrid Mount 的从零重建版本，基于以下行为参考：

- **Magic Mount**：参考 [`Tools-cx-app/meta-magic_mount-rs`](https://github.com/Tools-cx-app/meta-magic_mount-rs)（master `8b85c9e`；PR #152 暂不采纳）。
- **OverlayFS**：参考 Hybrid Mount v4.2.0（tag `e20f9c19`）的 overlay 行为。
- **WebUI**：Vue 3 双 UI —— Miuix（默认） + MD3（保留现有界面体验）。
- **前后端交互**：`kernelsu.exec` 直调 CLI，无 daemon / HTTP / SSE。
- **模块目录铁律**：任何阶段不得移动、合并或删除
  `/data/adb/modules/<id>/system/**`。

> 从 v6.0.0 起，`dev` 是从零重建后的开发主线。v4.2.0 及更早实现仍可通过
> release tag、`archive/*` 分支和完整 Git 历史查阅，但不再混入当前代码树。

## 语言 / Languages

- [简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/README.md)
- [Türkçe](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_TR.md)

## 历史与贡献者

新的 `dev` 代码树不再携带旧 daemon、Kasumi、Lite/Nano flavor 和 React WebUI，
但通过 Git 历史保留合并连接旧开发线。因此，以前的提交、作者和贡献记录仍然
完整可追溯；WebUI 的贡献者列表也继续从仓库贡献历史动态生成并排除机器人。

安装和反馈问题前请阅读 [`USAGE_NOTICE.md`](USAGE_NOTICE.md)。

## 许可证

- 核心（Rust、module 脚本）：GPL-3.0-only（见 `LICENSE`）。
- WebUI：Apache-2.0（见 `webui/LICENSE`）。

## 规划

架构决策和原始实施计划见本仓库 `REHYBRID_MOUNT_PLAN.md`。
