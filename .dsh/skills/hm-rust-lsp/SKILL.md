---
name: hm-rust-lsp
description: （当前不可用）在 DeepSeek Harness 中用 dsh-lsp-actions 与 rust-analyzer 检查 Rust 诊断、符号、类型提示和重命名；用于 Rust 编辑与语义重构。
---

# DSH Rust LSP

**当前不可用**：`dsh-lsp-actions` 声明 `engines.dsh >=0.1.2-rc.1 <0.2.0`，与本机 DSH 0.2.0-rc.2 不兼容，插件装不上（见 `.dsh/plugins.md`），所以下面的 `lsp_*` 工具在当前会话中并不存在；本机也没有 `web` profile（活动 profile 是 `desktop`）。保留本技能作参考，等上游支持 0.2.x 后再启用。

- lsp_symbols 查看文件符号或查找工作区符号；lsp_inlay_hints 辅助理解推断类型；lsp_signature 查看调用参数。
- 修改后用 lsp_diagnostics 检查相关文件；首次索引未完成或服务器超时时，不能把空结果解释为无错误。
- lsp_code_action 返回修复建议，审阅改动再应用；lsp_rename 会修改工作区文件，执行前确认范围，完成后检查 git diff，保留用户已有改动。
- lsp_format 会写文件；仅检查格式时使用 cargo fmt --all -- --check。
- 若当前工具目录有内置定义/引用导航，使用其实际 schema；动作插件本身不提供这两种工具。
- 插件可用后：用 `dsh --dump-config`（当前 profile）确认 lsp-actions 的 servers.rust，并检查 `rust-analyzer --version`。新增 bundle 后需重启 DSH，新会话重新发现技能。
- Rust LSP 的宿主机诊断不能覆盖 Android cfg 分支；平台改动继续执行 hm-rust-android 的目标检查。
