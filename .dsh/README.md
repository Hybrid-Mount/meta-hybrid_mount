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
| rust-skills | 265 条通用 Rust 规则库（26 类，按需展开单个规则；已加仓库优先级说明） |
| hm-rust-android | Android 三架构、cfg 与宿主机验证边界 |
| hm-rust-lsp | DSH Rust 诊断、符号与重命名工具使用 |
| hm-webui | WebUI 开发与 kernelsu.exec JSON 命令协议 |

注意：`.claude/skills/` 是 Claude Code 用的，且已在 `.gitignore` 中；
DSH **不会**读取该目录，两边各自维护。

`rust-skills` 与其他 Rust 相关 skill 的分工：它提供通用的逐条规则与反例，
`rust-best-practices` 提供成体系的编码指南，`hm-*` 提供本仓库的门禁与不变量。
三者冲突时以 `hm-*` 与仓库根 `CLAUDE.md` 为准。

## 升级 vendored skill

`.dsh/skills/rust-skills/` 是 [leonardomso/rust-skills](https://github.com/leonardomso/rust-skills)
（MIT）的原样快照，只改了 `SKILL.md` 的 frontmatter 与开头的仓库优先级说明。
`SKILL.md` 的 `metadata.vendored` 记录了来源 commit。

```bash
git clone --depth 1 https://github.com/leonardomso/rust-skills /tmp/rust-skills
cp -R /tmp/rust-skills/rules .dsh/skills/rust-skills/rules
cp /tmp/rust-skills/LICENSE .dsh/skills/rust-skills/LICENSE
cp /tmp/rust-skills/checks/validate.py .dsh/skills/rust-skills/checks/
# 上游 SKILL.md 是规则索引，需要合并而不是整份覆盖
diff /tmp/rust-skills/SKILL.md .dsh/skills/rust-skills/SKILL.md
python3 .dsh/skills/rust-skills/checks/validate.py   # 索引/链接/孤儿规则完整性
```

先 `diff` 上游 `SKILL.md`，把新增/删除的规则条目同步进本仓库副本；
然后把本仓库的 frontmatter（`whenToUse`、`metadata.vendored`）与
“Hybrid Mount precedence”一节补回，并更新记录的 commit。
`validate.py` 会检查索引与 `rules/` 是否严格一一对应，是这一步的验收命令。

## 插件

Web profile 的插件不在这里配置，而在 `~/.dsh/profiles/web/`：

```bash
dsh plugin --profile web add <package>     # 安装
dsh plugin --profile web remove <package>  # 卸载
dsh --profile web --dump-config            # 验证组合后的配置树
```

为这个仓库安装的插件记录在仓库根 `.dsh/plugins.md`。
