# 为 Hybrid Mount 安装的 DSH 插件

插件是 **profile 级**配置，对本机所有项目生效，不是仓库级配置。本机当前使用的 profile 是
`desktop`（`~/.dsh/profiles/desktop`），不是 `web`——下面的记录以实际 profile 为准。

## 由本人在 GUI 安装（agent 不代劳）

插件管理器会拒绝在 agent 会话运行期间安装（`dshmarket` 的
`.dsh-market/log.ndjson` 里可见 `install-blocked: refused while agents are running`），
因为并发写 `package.json` / `node_modules` 会互相覆盖。所以约定：**插件一律由人在 GUI 里装，
agent 只负责给兼容性结论，不改 profile。**

| 插件 | 版本 | 用途 | 为什么适合本仓库 |
| --- | --- | --- | --- |
| `dshmarket` | 1.66.5 | 可视化插件市场 | 已装；后续插件的安装入口 |
| `@linxin666/dsh-client-ui-skill-explorer` | 0.4.4 | 按来源（bundled/project/user/custom/runtime）浏览、启停、增删 skill | 本仓库有 15 个 `.dsh/skills/`，需要面板管理 |
| `@linxin666/dsh-client-ui-git-graph` | 0.4.4 | 提交图谱 | 仓库 git 历史密集，`dev`/`main` 双分支频繁合并 |

### 已核实不兼容，不要装

判断依据是 npm 上的 `engines.dsh` / `peerDependencies`，与 app 内置的
`@deepseek-ai/dsh-*@0.2.0-rc.2`（解包自 `app.asar`）逐个比对：

| 插件 | 声明要求 | 结论 |
| --- | --- | --- |
| `dsh-lsp-actions` | `>=0.1.2-rc.1 <0.2.0`（含全部备选区间） | 排除 0.2.0-rc.2，**装不上** |
| `@wenaixi/dsh-superpower` | `@deepseek-ai/dsh-skill >=0.0.1-rc.1 <0.2.0-0` | 排除 0.2.0-rc.2，**装不上** |

> `dsh-lsp-actions` 装不上直接影响 `.dsh/skills/hm-rust-lsp/`：那个 skill 描述的 `lsp_*`
> 工具在当前 profile 里并不存在。改 Rust 时不要依赖它，用 `cargo clippy` / `rust-analyzer`
> 自身的方式验证。等上游放出支持 0.2.x 的版本后再装，并同步更新该 skill。

安装前务必核对兼容性，命令：

```bash
# 以桌面 app 内置版本为准（0.2.0-rc.2），而不是 npm 上 @deepseek-ai/dsh 的 latest
npm view <package> engines peerDependencies
```

## 回滚

本机 profile 的 `package.json` / `cordis.yml` / `cordis.patch.yml` / `pnpm-workspace.yaml` /
`pnpm-lock.yaml` 已备份到：

```
~/.dsh/profiles/desktop/.hybrid-mount-backup-20260930/
```

在 GUI 里装插件若导致 app 起不来，覆盖回去再重启即可。profile 目录内的
`package.json.lock` 是插件管理器的互斥锁，正常情况下随操作结束释放。

## 管理命令（需要 DSH 自带 CLI）

app 目录里没有可直接调用的 `dsh`，用 npm 上的同版本 CLI：

```bash
pnpm add --dir /tmp/dsh-cli @deepseek-ai/dsh@0.2.0-rc.2
node /tmp/dsh-cli/node_modules/@deepseek-ai/dsh/lib/bin.js plugin --profile desktop add <package>
node /tmp/dsh-cli/node_modules/@deepseek-ai/dsh/lib/bin.js --profile desktop --dump-config
```

CLI 版本必须与 app 内置的 `@deepseek-ai/dsh-*` 版本一致（当前 **0.2.0-rc.2**），
否则组合出来的 profile 树可能与 app 实际加载的不符。新装 bundle 需要**重启 DSH** 才生效。

## 历史

早期本文件记录的是 `web` profile 上的安装（`dshmarket`、`@wenaixi/dsh-superpower`、
`dsh-lsp-actions`、`@furongjun1999/dsh-memory`、`@linxin666/dsh-client-ui-skill-explorer`）
与 rust-analyzer 的 lsp-actions 配置。本机现在没有 `web` profile，且其中两个插件与
0.2.0-rc.2 不兼容，因此该记录仅作历史参考，不再代表可用状态。
