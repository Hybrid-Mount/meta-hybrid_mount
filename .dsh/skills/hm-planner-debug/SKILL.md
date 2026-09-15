---
name: hm-planner-debug
description: 诊断 Hybrid Mount 规划器冲突（同路径多后端、.replace 与 Overlay 后代、文件/目录类型冲突），并给出在 plan 阶段的修复方式与回归测试做法。
whenToUse: 出现 plan conflict、挂载未生效、需要判断某路径走 overlay 还是 magic，或需要为冲突补测试时。
---

# 规划器冲突诊断

规划阶段（`src/plan/mod.rs`）在 executor 之前把跨模块冲突显式暴露。
**修复必须在 planner，而不是在执行器里兜底。**

## 先确认规则命中

优先级：`路径规则 > 模块 default_mode > 全局 default_mode`

- 配置：`/data/adb/hybrid-mount/config.toml`
- `hybrid-mount show-config` 查看当前配置，`gen-config` 生成模板。
- `hybrid-mount modules` 查看模块列表（快照 `/data/adb/hybrid-mount/scan.ret`）。

## 常见冲突

1. **同一目标路径进入两个后端**
   module A 把 `system/etc/hosts` 判为 overlay，module B 判为 magic。
   → 用路径规则统一到同一后端，或把其中一个模块设为 `default_mode = "ignore"`。

2. **Magic .replace 与 Overlay 后代**
   .replace 会替换整个目录，不能再包含 overlay 子挂载。
   → 整个目录统一后端，或去掉 .replace。

3. **类型冲突 / 同路径重复**
   同一路径被两个模块当文件，或目录 vs 文件。
   → 在 planner 明确取舍，不要依赖挂载顺序。

## 定位与复现

相关实现：

- `src/plan/mod.rs` — 规则与冲突检测
- `src/mount_tree.rs` — 统一节点树（`MountNode` / `MountSource`）
- `src/pipeline.rs` — 执行顺序与回滚
- `src/state.rs` — `run/state.json` 失败快照

```bash
cargo test -p hybrid-mount plan -- --nocapture
```

新增冲突类型时同步补一条 planner 单测（模块末尾 `#[cfg(test)] mod tests`）。

## 排查顺序

1. `show-config` 确认规则与 default_mode；
2. 确认模块条目的类型（文件 / 目录 / .replace）；
3. 对照上表判断冲突类别；
4. 改配置或 planner，重跑 hm-verify。
