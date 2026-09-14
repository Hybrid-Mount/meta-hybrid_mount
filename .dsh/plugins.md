# 为 Hybrid Mount 安装的 DSH 插件

安装在 **web profile**（`~/.dsh/profiles/web`）。插件是 profile 级、对本机所有项目生效，不是仓库级配置。

| 插件 | 版本 | 用途 | 为什么适合本仓库 |
| --- | --- | --- | --- |
| dsh-lsp-actions | 0.5.1 | 通过真实语言服务器提供 diagnostics / format / code actions / symbols / rename 等 `lsp_*` 工具 | 主体是 Rust，另有 Vue/TS 前端；本机已装 rust-analyzer |
| @wenaixi/dsh-superpower | 6.3.1 | obra/superpowers 的 DSH 移植，14 个方法论 skill（规划、TDD、系统化调试、评审、完成前验证） | 与仓库严格的 lint / CI 门禁互补 |

原来已有（保持不动）：`dshmarket`、`@furongjun1999/dsh-memory`、`@linxin666/dsh-client-ui-skill-explorer`。

## 管理命令

```bash
dsh plugin --profile web add <package>       # 安装（自动登记到 dsh.profile.bundles）
dsh plugin --profile web remove <package>    # 卸载
dsh --profile web --dump-config              # 查看组合后的配置树
```

## 生效方式

新增 bundle 需要**重启 `dsh web`** 后才会被加载；当前运行的进程仍是安装前的组合。
重启后：

- `lsp_*` 工具可用（Rust 侧依赖本机 `rust-analyzer`，已确认存在）；
- 14 个 `superpower-*` skill 进入 skill catalog。

## 回滚

安装前已备份清单文件到 `~/.dsh/profiles/web/.hybrid-mount-backup-20260914/`
（`package.json`、`pnpm-lock.yaml`、`cordis.yml`、`cordis.patch.yml`）。
需要还原时覆盖回去，再执行 `dsh plugin --profile web install`。

## Rust 支持补充（2026-09-14）

核实上述两个插件均已安装。发现 lsp-actions 默认 servers 为空，现已在 web profile 的 cordis.patch.yml 配置 rust-analyzer（仅 .rs）；编辑器 stdio 模式保持关闭。配置修改前备份：`~/.dsh/profiles/web/.rust-support-backup-20260914-182012`。

项目 skills 新增 rust-best-practices（来源 https://github.com/apollographql/skills/tree/main/skills/rust-best-practices，MIT；移除 Claude 工具限制并补充本仓库 lint 优先级）、hm-rust-android 和 hm-rust-lsp。保留上游 references 与 LICENSE。新会话重新发现 skills；如当前进程未加载插件，重启 dsh web。
