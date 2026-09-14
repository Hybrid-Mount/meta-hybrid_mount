# .dsh — DeepSeek Harness 项目配置

本目录让 DSH 在这个仓库里开箱即得项目知识与门禁流程。

## skills/

DSH 会扫描项目根的 `.dsh/skills/`（rank 100，优先级最高）与 `.agents/skills/`。
这里的每个子目录是一个 skill 包，入口为 `SKILL.md`（YAML frontmatter 需含 `name` 与 `description`）。

| Skill | 用途 |
| --- | --- |
| hybrid-mount-overview | 架构、流水线、规则优先级、不变量、禁止事项 |
| hm-verify | 对齐 .github/workflows/lints.yml 的本地全量校验 |
| hm-build-release | cargo xtask 构建、update.json、changelog、发布流程 |
| hm-planner-debug | 规划器冲突诊断与回归测试 |
| hm-mount-safety-review | 挂载/回滚相关代码的审查清单 |
| rust-best-practices | Apollo Rust 编码、所有权、错误处理与性能指南（已适配仓库 lint） |
| hm-rust-android | Android 三架构、cfg 与宿主机验证边界 |
| hm-rust-lsp | DSH Rust 诊断、符号与重命名工具使用 |
| hm-webui | WebUI 开发与 kernelsu.exec JSON 命令协议 |

注意：`.claude/skills/` 是 Claude Code 用的，且已在 `.gitignore` 中；
DSH **不会**读取该目录，两边各自维护。

## 插件

Web profile 的插件不在这里配置，而在 `~/.dsh/profiles/web/`：

```bash
dsh plugin --profile web add <package>     # 安装
dsh plugin --profile web remove <package>  # 卸载
dsh --profile web --dump-config            # 验证组合后的配置树
```

为这个仓库安装的插件记录在仓库根 `.dsh/plugins.md`。
