# ReHybrid-Mount

ReHybrid-Mount 是 Hybrid Mount 的从零重建版本，基于以下行为参考：

- **Magic Mount**：参考 [`Tools-cx-app/meta-magic_mount-rs`](https://github.com/Tools-cx-app/meta-magic_mount-rs)（master `8b85c9e`；PR #152 暂不采纳）。
- **OverlayFS**：参考 Hybrid Mount v4.2.0（tag `e20f9c19`）的 overlay 行为。
- **WebUI**：Vue 3 双 UI —— Miuix（默认） + MD3（保留现有界面体验）。
- **前后端交互**：`kernelsu.exec` 直调 CLI，无 daemon / HTTP / SSE。
- **模块目录铁律**：任何阶段不得移动、合并或删除
  `/data/adb/modules/<id>/system/**`。

> 当前分支 `rehybrid-mount` 是 orphan 分支，正在从零重建。旧实现保留在原
> `main` / `dev` / `archive/*` 分支中。

## 许可证

- 核心（Rust、module 脚本）：GPL-3.0-only（见 `LICENSE`）。
- WebUI：Apache-2.0（见 `webui/LICENSE`）。

## 规划

完整实施计划见旧仓库工作树中的 `REHYBRID_MOUNT_PLAN.md`（将随骨架阶段迁入本分支）。
