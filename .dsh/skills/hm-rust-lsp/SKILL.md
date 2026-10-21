---
name: hm-rust-lsp
description: 在 DeepSeek Harness 中用 dsh-lsp-actions 与 rust-analyzer 检查 Rust 诊断、符号、类型提示和重命名；用于 Rust 编辑与语义重构。
---

# DSH Rust LSP

使用前检查当前会话是否提供 `lsp_*` 工具，以及插件与 DSH 版本是否兼容；不可用时改用 Cargo 检查。

- lsp_symbols 查看文件符号或查找工作区符号；lsp_inlay_hints 辅助理解推断类型；lsp_signature 查看调用参数。
- 修改后用 lsp_diagnostics 检查相关文件；首次索引未完成或服务器超时时，不能把空结果解释为无错误。
- lsp_code_action 返回修复建议，审阅改动再应用；lsp_rename 会修改工作区文件，执行前确认范围，完成后检查 git diff，保留用户已有改动。
- lsp_format 会写文件；仅检查格式时使用 cargo fmt --all -- --check。
- 若当前工具目录有内置定义/引用导航，使用其实际 schema；动作插件本身不提供这两种工具。
- 插件可用后：用 `dsh --dump-config`（当前 profile）确认 lsp-actions 的 servers.rust，并检查 `rust-analyzer --version`。新增 bundle 后需重启 DSH，新会话重新发现技能。
- Rust LSP 的宿主机诊断不能覆盖 Android cfg 分支；平台改动继续执行 hm-rust-android 的目标检查。
