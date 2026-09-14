---
name: hm-rust-lsp
description: 在 DeepSeek Harness 中用 dsh-lsp-actions 与 rust-analyzer 检查 Rust 诊断、符号、类型提示和重命名；用于 Rust 编辑与语义重构。
---

# DSH Rust LSP

本机 web profile 已为 dsh-lsp-actions 配置 rust-analyzer，匹配 .rs，项目标记为 Cargo.toml/rust-project.json。使用当前 DSH 工具目录中的真实 schema，不使用 Codex 的 rust-lsp MCP 工具名。

- lsp_symbols 查看文件符号或查找工作区符号；lsp_inlay_hints 辅助理解推断类型；lsp_signature 查看调用参数。
- 修改后用 lsp_diagnostics 检查相关文件；首次索引未完成或服务器超时时，不能把空结果解释为无错误。
- lsp_code_action 返回修复建议，审阅改动再应用；lsp_rename 会修改工作区文件，执行前确认范围，完成后检查 git diff，保留用户已有改动。
- lsp_format 会写文件；仅检查格式时使用 cargo fmt --all -- --check。
- 若当前工具目录有内置定义/引用导航，使用其实际 schema；动作插件本身不提供这两种工具。
- 工具缺失时检查 dsh --profile web --dump-config 中 lsp-actions 的 servers.rust；检查 rust-analyzer --version。新增 bundle 后需重启 DSH，新会话重新发现技能。
- Rust LSP 的宿主机诊断不能覆盖 Android cfg 分支；平台改动继续执行 hm-rust-android 的目标检查。
