# .dsh — DeepSeek Harness 项目配置

本目录让 DSH 在这个仓库里开箱即得项目知识与门禁流程。

## skills/

DSH 会扫描项目根的 `.dsh/skills/`（rank 100，优先级最高）与 `.agents/skills/`。
这里的每个子目录是一个 skill 包，入口为 `SKILL.md`（YAML frontmatter 需含 `name` 与 `description`）。

### 本仓库自建（`hm-*` 与总览）

| Skill | 用途 |
| --- | --- |
| hybrid-mount-overview | 架构、流水线、规则优先级、不变量、禁止事项 |
| hm-verify | 对齐 .github/workflows/lints.yml 的本地全量校验 |
| hm-build-release | cargo xtask 构建、update.json、changelog、发布流程 |
| hm-planner-debug | 规划器冲突诊断与回归测试 |
| hm-mount-safety-review | 挂载/回滚相关代码的审查清单 |
| hm-vfs | VFS 内核子系统 hybridmount：module/vfs 内核模块 + src/vfs 控制面、GKI 选型、plan 前加载约束、sources.txt 防漂移 |
| hm-runtime | src/runtime 运行时层：账本阶段机、热挂载与 boot lock 竞态、事务回滚清理、状态快照契约 |
| hm-module-scripts | module/*.sh 与 tests/shell/*.sh：触发时机、可用变量、退出码语义、平台裁剪 |
| hm-ci | 8 个 workflow 的触发矩阵、权限、产物回写与发布链路 |
| hm-rust-android | Android 三架构、cfg 与宿主机验证边界 |
| hm-rust-lsp | DSH Rust 诊断、符号与重命名工具使用（**当前不可用**，见下） |
| hm-webui | WebUI 开发与 kernelsu.exec JSON 命令协议 |

### vendored 上游 skill

来源、commit 与许可都记在各 `SKILL.md` 的 `metadata.vendored` 里，正文开头统一有
“Hybrid Mount precedence”一节，声明本仓库契约优先于通用规则。

| Skill | 上游 | 用途与本地化改动 |
| --- | --- | --- |
| rust-skills | [leonardomso/rust-skills](https://github.com/leonardomso/rust-skills) | 265 条通用 Rust 规则库（26 类，按需展开）；加仓库优先级说明 |
| rust-best-practices | [apollographql/skills](https://github.com/apollographql/skills) | Apollo Rust 编码、所有权、错误处理与性能指南；移除 Claude 工具限制、适配仓库 lint |
| vue-best-practices | [vuejs-ai/skills](https://github.com/vuejs-ai/skills) | WebUI 的 Vue 3 + TS 组件、响应式与组件边界；声明本仓库无 Router/Pinia/JSX/SSR |
| vue-testing-best-practices | [vuejs-ai/skills](https://github.com/vuejs-ai/skills) | WebUI 的 Vitest + Vue Test Utils 测试；剔除 Playwright/E2E 与 Pinia 内容 |
| kernel-development-skills | [UtsavBalar1231/kernel-development-skills](https://github.com/UtsavBalar1231/kernel-development-skills) | 内核侧 C（module/vfs/src、module/lkm/src）的驱动 API、调试与 patch 清单；剔除 RK3576/Lapis 板级文件与 Codex 配置 |

上游同源但**故意未 vendor** 的：`vue-router-*`、`vue-pinia-*`、`vue-jsx-*`、
`vue-options-api-*`（webui 不用这些技术）、`vue-debug-guides`（621 KB，相对 406 KB 的
`webui/src` 过重）。需要时按下面的升级步骤单独取。

注意：`.claude/skills/` 是 Claude Code 用的，且已在 `.gitignore` 中；
DSH **不会**读取该目录，两边各自维护。

`hm-rust-lsp` 当前**不可用**：它依赖的 `dsh-lsp-actions` 插件声明
`engines.dsh = ">=0.1.2-rc.1 <0.2.0"`，而本机 DSH 内置的是 `0.2.0-rc.2`，装不上。
详见 `.dsh/plugins.md`；在该插件支持 0.2.x 之前，Rust 改动请用 `cargo clippy` 验证。

`rust-skills` 与其他 Rust 相关 skill 的分工：它提供通用的逐条规则与反例，
`rust-best-practices` 提供成体系的编码指南，`hm-*` 提供本仓库的门禁与不变量。
三者冲突时以 `hm-*` 与仓库根 `CLAUDE.md` 为准。

## 升级 vendored skill

每个 vendored skill 的 `SKILL.md` 里 `metadata.vendored.{source,commit,upstream-path}`
记录了取用来源。升级流程固定为「取上游 → diff → 合并而不是覆盖」：

```bash
git clone --depth 1 https://github.com/vuejs-ai/skills /tmp/vue-skills
diff -r /tmp/vue-skills/skills/vue-best-practices .dsh/skills/vue-best-practices
```

对 `rust-skills` 另有上游自带的完整性校验：

```bash
git clone --depth 1 https://github.com/leonardomso/rust-skills /tmp/rust-skills
diff /tmp/rust-skills/SKILL.md .dsh/skills/rust-skills/SKILL.md
python3 .dsh/skills/rust-skills/checks/validate.py   # 索引/链接/孤儿规则完整性
```

合并时要保住本仓库加的三样东西：中文 `description`、`whenToUse`、
“Hybrid Mount precedence”一节，以及被删掉的上游文件清单（`metadata.vendored.trimmed`）。
上游 `SKILL.md` 通常是索引，必须逐条合并，不要整份覆盖。

自检（改了任何 skill 都跑一遍）：

```bash
node .dsh/validate-skills.mjs
```

它只依赖 Node 内置模块，检查三件事：每个技能目录都有可解析的 `SKILL.md` frontmatter、
frontmatter 里的 `name` 与目录名一致、以及技能内所有带扩展名的相对 Markdown 链接都指向
真实文件。退出码非零即有问题，逐条打印 `FAIL` 行。

> 注意：不要用 PowerShell 的 `Set-Content` 回写这些 Markdown 文件——它的默认编码不是
> UTF-8，会把中文写成替换字符，且写坏后的文件连读取工具都会拒绝。要改就用编辑器工具，
> 或以 `[System.IO.File]::WriteAllText($p, $s, [System.Text.UTF8Encoding]::new($false))` 写。

## 插件

插件是 profile 级配置，本机用的是 `desktop` profile（不是 `web`）。**约定：插件由人在 GUI 里安装**，
agent 不改 profile——插件管理器本身就会拒绝在 agent 会话运行期间安装。
详见 `.dsh/plugins.md`（含兼容性核查方法与已确认装不上的插件）。

```bash
dsh plugin --profile desktop add <package>     # 安装
dsh plugin --profile desktop remove <package>  # 卸载
dsh --profile desktop --dump-config            # 验证组合后的配置树
```

app 目录里没有可直接调用的 `dsh`；用 npm 上同版本的 CLI（当前 `0.2.0-rc.2`），
命令见 `.dsh/plugins.md`。新装 bundle 需要重启 DSH 才生效。
