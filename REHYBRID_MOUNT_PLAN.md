# ReHybrid-Mount 重构计划

> 目标位置：**同一仓库 `Hybrid-Mount/meta-hybrid_mount`，新建空白 orphan branch（默认名 `rehybrid-mount`）**
> 状态：**执行中** — Stage 0 已完成（orphan 分支 `rehybrid-mount` + 初始骨架已推送）；后续按 Stage 1 → 9 实施
> 原则：**完全从 0 开始**；旧历史与参考项目只作为行为规范与资料，不作为代码基线。

---

## 0. 已确定的关键决定

| # | 事项 | 决定 |
|---|---|---|
| 1 | 仓库起点 | 同一仓库内的**空白 orphan branch**，所有实现代码从 0 新写；旧分支不动 |
| 2 | 前端-后端交互 | 彻底移除 daemon/HTTP/SSE，使用 `kernelsu.exec` 直调 CLI |
| 3 | Magic Mount | 算法/行为参考 `meta-magic_mount-rs` master `8b85c9e`，代码新写；**上游 PR #152 暂不采纳** |
| 4 | OverlayFS | 行为参考 **v4.2.0 tag `e20f9c19`**，代码新写 |
| 5 | 混合策略 | 保留模块级模式 + 路径规则：`overlay / magic / ignore` |
| 6 | Kasumi | 全仓 0 引用（源码、WebUI、module、locale、docs、CI 全部无 Kasumi） |
| 7 | 模块目录铁律 | 任何阶段不得移动/合并/删除 `/data/adb/modules/<id>/system/**` |
| 8 | WebUI | **Vue 3 双 UI：Miuix 默认 + MD3 可选**；MD3 模式复刻现有界面体验；代码全新编写 |
| 9 | CI / Release | **唯一例外**：按现有行为重新实现并连线，含 KSU 模块仓库与 TG 群组 |
| 10 | 语言提交 | 从备份分支找回原始提交（唯一允许的历史内容取回） |

---

## 1. 目标架构

```
KernelSU WebUI（全新编写的 Vue 3 双 UI：Miuix 默认 / MD3 保留）
        │  ksu.exec()
        ▼
/data/adb/modules/hybrid_mount/hybrid-mount   ← 唯一二进制
        │ show-config / save-config --payload <hex>
        │ modules / status / version / install-state
        │ clear-mount-errors / emulated-soft-reboot
        ▼
/data/adb/hybrid-mount/
  ├─ config.toml      ← 持久配置（含 rules 路径规则）
  ├─ scan.ret         ← 启动时生成的模块快照
  └─ run/state.json   ← 启动时生成的挂载状态快照（替代 daemon 实时状态）
```

启动流程（无参数）：

```
metamount.sh → hybrid-mount
  → 读取 config.toml
  → 只读扫描模块
  → 混合 planner：模块级模式 + 路径规则
  → overlayfs 执行（v4.2.0 语义）
  → magic mount 执行（参考项目算法，只处理 magic 选中的模块）
  → bind mount 自定义列表（参考项目行为）
  → 写 scan.ret + run/state.json
```

---

## 2. 从 0 重写原则

1. 不 checkout 旧分支、不 subtree、不 cherry-pick 任何实现代码提交。
2. 旧分支仍在同一仓库，只用于：
   - 查阅 v4.2.0 overlay 行为；
   - 找回第 7 节的语言文件提交。
3. 参考项目只保留为 `upstream` 只读 remote，用于查阅 magic mount 行为与交互契约。
4. CI/Release YAML 与 `tools/notify` 按现有行为重新实现，这是唯一允许“继承现有连线”的部分。

---

## 3. 新分支目录布局

```
/
├─ Cargo.toml / Cargo.lock / build.rs / rust-toolchain.toml / clippy.toml
├─ src/
│  ├─ main.rs          # 无参数挂载；有参数 CLI
│  ├─ defs.rs          # 路径/常量（无 Kasumi、无 daemon）
│  ├─ errors.rs
│  ├─ config.rs        # TOML 配置 + show/save/gen-config
│  ├─ scanner.rs       # 只读模块扫描
│  ├─ parser.rs        # ignore/bind/file 自定义列表
│  ├─ bind_mount.rs
│  ├─ magic_mount/     # 参考项目算法，新实现 + 模块选择扩展
│  ├─ overlayfs/       # v4.2.0 行为，新实现
│  ├─ storage/         # ext4 / tmpfs staging
│  ├─ plan/            # 混合路径规则 planner
│  ├─ sys/             # mount/fs/nuke 辅助
│  └─ utils/
├─ webui/              # 全新 Vue 3 双 UI：Miuix 默认 + MD3 保留
├─ module/             # 安装脚本：symlink-only 分区处理
├─ xtask/              # 打包 + notify 命令
├─ tools/notify/       # TG 通知 helper（重新实现现有行为）
├─ tests/
├─ docs/ + 多语言 README
├─ changelog.md / update.json
└─ .github/            # CI / Release / 门禁
```

---

## 4. 配置契约

### 4.1 config.toml

```toml
moduledir = "/data/adb/modules"
mountsource = "KSU"
overlay_mode = "ext4"      # tmpfs | ext4（v4.2.0 语义）
disable_umount = false
default_mode = "overlay"   # overlay | magic | ignore

[rules."<module_id>"]
default_mode = "magic"

[rules."<module_id>".paths]
"system/etc/hosts" = "overlay"
```

### 4.2 CLI 契约（参考项目式）

| 命令 | 行为 | WebUI 使用 |
|---|---|---|
| （无参数） | 完整挂载流水线 | `metamount.sh` |
| `show-config` | 输出 JSON 配置 | `loadConfig` |
| `save-config --payload <hex>` | 合并/持久化配置，返回 `{ok:true}` | `saveConfig` / 模块规则保存 |
| `gen-config` | 重置默认配置 | `resetConfig` |
| `modules` | 输出启动时缓存的 `scan.ret` | `scanModules` |
| `status` | 输出 `run/state.json` | `getStorageUsage` |
| `version` | 输出 `{"version": "..."}` | `getVersion` |
| `install-state` | 安装兼容性状态 | 启动门 / 干净重装提示 |
| `clear-mount-errors` | 清除模块 `mount_error` 标记 | 模块页按钮 |
| `emulated-soft-reboot` | 软重启处理 | 参考项目行为 |

### 4.3 模块目录不可变原则（硬性门禁）

1. 安装阶段分区处理**只允许**：
   ```sh
   ln -sf "./system/$partition" "$MODPATH/$partition"
   ```
   禁止 `cp -a ... && rm -rf`、禁止 `mv system/<partition>`、禁止 normalize。
2. v5 的 `normalize_symlinked_partition_layout` 与旧历史的 `normalize_module_layout` 均为禁用逻辑，CI 黑名单检查。
3. 扫描 / 规划 / 执行阶段：模块源目录只读；staging 只写入 `/data/adb/hybrid-mount`。
4. magic mount 收集只 read + bind mount，绝不写回模块目录。
5. overlay lowerdirs 直接指向模块原始路径（v4.2.0 行为）。
6. 回归测试：
   - 安装脚本测试：含 `system/product/app` 的模块执行分区处理后，原目录仍在、`product` 只是 symlink。
   - Rust 测试：含 `system/product/*` 的 fixture 走 scan+plan 后源目录不变。
   - CI 黑名单：源码不得出现对 `system/{vendor,product,system_ext}` 的 `mv/rm/cp` 归一化逻辑。

---

## 5. 实施阶段

### Stage 0 — 同仓库空白 orphan branch 与工程骨架
- 在当前仓库直接创建空白分支（旧分支完全不动）：
  ```bash
  git remote add upstream https://github.com/Tools-cx-app/meta-magic_mount-rs.git
  git fetch upstream master
  git switch --orphan rehybrid-mount
  git rm -rf .
  # 写入全新骨架：LICENSE / .gitignore / rust-toolchain.toml / README / CI
  git add -A
  git commit -m "chore: init ReHybrid-Mount from scratch"
  git push -u origin rehybrid-mount
  ```
- 旧历史仍在同一个对象库里，语言提交 cherry-pick、v4.2.0 行为查阅、CI/Release 参考都不需要跨仓库。
- 初始提交：LICENSE（核心 GPL-3.0-only，WebUI Apache-2.0）、`.gitignore`、`rust-toolchain.toml`、README 骨架、CI 骨架。
- 项目身份：module id `hybrid_mount`，模块名 **Hybrid Mount**，运行目录 `/data/adb/hybrid-mount`；分支/产物品牌为 ReHybrid-Mount。
- 默认分支暂保持旧 `main`；`rehybrid-mount` 为唯一开发分支，稳定后按第 7 节做分支晋升。

### Stage 1 — 核心基础（新写）
- `defs.rs`、`errors.rs`、日志初始化、panic hook。
- `config.rs` TOML schema + 默认值 + 单元测试。

### Stage 2 — magic mount（新写，行为参考 `8b85c9e`）
- Node 树、`RegularFile / Directory / Symlink / Whiteout`。
- `module.prop`、`disable / remove / skip_mount`、`system` 目录、内建分区提升规则。
- 目录/文件/符号链接/whiteout 挂载语义、`.replace` xattr、tmpfs skeleton、mirror、只读 remount。
- 混合扩展：只收集 planner 判为 magic 的模块/路径。
- KernelSU unmount 列表集成。

### Stage 3 — overlayfs + storage（新写，行为参考 v4.2.0 `e20f9c19`）
- fsopen(`overlay`) 主路径 + 传统 `mount` fallback。
- lowerdir 转义、>64 层 staging、`/proc/self/mountinfo` 子挂载处理。
- tmpfs / ext4 loop image staging，`mkfs.ext4` / `e2fsck`、SELinux context。
- 单元测试：lowerdir 转义、层数拆分、子挂载路径计算。

### Stage 4 — 混合 planner（新写）
- 规则优先级：路径规则 > 模块 default_mode > 全局 default_mode。
- 同一路径只进入一个后端；冲突启动时显式报错。
- overlay lowerdirs 按目标分区聚合；magic 模块 id 列表传给 Stage 2。
- 模块目录不可变回归测试。

### Stage 5 — CLI（新写，参考项目交互契约）
- 手工参数解析，无 clap 运行时依赖。
- `--payload` 使用 hex 编码 JSON，完全按参考项目方式。
- `scan.ret` / `run/state.json` 生成与读取。

### Stage 6 — WebUI（Vue 3 双 UI：Miuix 默认 + MD3 保留）
- 技术栈：**Vue 3 + Vite + vue-i18n + `miuix-vue`（Miuix）/ 自定义 MD3 组件 + `@material/web`（MD3，按需）**。
- 架构完全采用上游已验证的双 UI 模式：
  ```
  webui/src/
  ├─ App.vue              # uiStore.uiStyle === "miuix" ? MiuixApp : Md3App
  ├─ lib/                 # 共享层，只写一份
  │  ├─ api.ts / api.mock.ts / types.ts / constants.ts
  │  ├─ stores/{config,module,sys,ui}Store.ts
  │  └─ locales/          # vue-i18n 消息
  └─ ui/
     ├─ md3/              # 保留现有 Material 界面体验
     │  ├─ App.vue + Md3Layout.vue
     │  ├─ pages/{status,config,modules,info}.vue
     │  └─ components/
     └─ miuix/            # 新默认风格，复用上游 miuix-vue 组件
        ├─ App.vue + MiuixLayout.vue
        ├─ pages/{status,config,modules,info}.vue
        └─ components/
  ```
- UI 风格设置：
  - 默认 `miuix`，`localStorage["uiStyle"]` 持久化，配置页可切换。
  - `App.vue` 用 `defineAsyncComponent` 动态加载当前 UI，只打包/加载激活的那一套页面。
  - Miuix 支持 Monet 取色开关（沿用上游 `uiStore.setMonetEnabled` 行为）。
- 两套 UI 都必须实现完整功能清单：
  - Status / Config / Modules / Info 四个页面
  - 配置字段：moduledir、mountsource、overlay_mode、disable_umount、default_mode、**路径规则编辑器**
  - 模块列表：搜索、过滤、模式标签、展开规则、mount_error / suggest_ignore、清空挂载错误
  - 重启确认、保存后“重启后生效”提示
  - 多语言与语言选择器
- 交互层新写：
  - 动态 `import("kernelsu")` → `ksu.exec`
  - 命令映射与 `--payload` hex 编码完全按参考项目方式
  - `api.mock.ts` 用于开发/测试
- 删除项：Kasumi 页面/字段/store、daemon 相关 UI、`daemon_startup_mode`。
- i18n 迁移：
  - 现有 11 个 locale JSON 转换为 vue-i18n 消息结构（key 语义保持不变）。
  - 语言选择器包含 `tr-TR` 与 `id-ID`。
- 验收：
  - MD3 模式对照旧 UI 视觉回归，做到交互与布局一致。
  - Miuix 模式对照上游交互规范做真机验证。
  - 两套 UI 共享同一数据层，禁止分叉 API/stores。

### Stage 7 — 找回备份分支的语言文件提交
1. 旧历史已在本仓库对象库中，直接按顺序 cherry-pick：
   - `d70c151e` → `webui/src/locales/tr-TR.json`（PR #379）
   - `da28a721` → `docs/README_TR.md`（PR #380）
   - `019799f8` → README 土耳其语链接（PR #381）
2. 冲突时回退方案：`git show <hash>:<path>` 取内容 + 原始 `--author` 提交。
3. 之后单独新写一个对齐提交：
   - 按最终 `en-US.json` key 结构校正 `tr-TR.json`
   - 更新 `README_TR.md` 描述新架构
   - 语言选择器包含 `tr-TR`（及 `id-ID`）
4. 校验：`git log --follow` 保留原提交；locale 结构与 `en-US` 一致。

### Stage 8 — module 脚本 / xtask / CI（见第 6 节详细方案）
- `metainstall.sh`：symlink-only 分区处理。
- `customize.sh` / `metamount.sh` / `uninstall.sh` 新写。
- `xtask`：构建、Vue WebUI 构建与 `MODULE_ID` 注入、zip 打包、update.json、notify。
- CI / Release：完整继承现有连线。

### Stage 9 — 验证
1. 主机侧：
   - Rust 单元测试：parser / config / scanner / planner / overlay / magic 树 / 布局不可变
   - WebUI：`pnpm lint`、`pnpm test`、`pnpm build`；双 UI 共用逻辑单元测试
   - 打包产物：无 kasumi、无 daemon、`system/*` 布局完整；Vue 双 UI 按需分块
2. Android 实机：
   - KSU / APatch 安装
   - magic mount：文件、目录、replace、whiteout
   - overlayfs：ext4、tmpfs、多层
   - 安装含 `system/product/*` 的模块 → `product` 是 symlink，原目录可继续编辑
   - 运行时编辑 `/data/adb/modules/<id>/system/*` → 重启后 magic mount 生效
   - WebUI：Miuix（默认）与 MD3 两种风格分别验证加载、切换、保存、模块列表、状态页

---

## 6. CI 与 Release 继承方案

> 因为继续使用老仓库，**不需要新配任何 secrets**：`RELEASE_TOKEN`、`TELEGRAM_BOT_TOKEN`、`TELEGRAM_CHAT_ID`、签名、GHCR 包权限全部沿用现有仓库配置。
> 行为对齐现有 `meta-hybrid_mount` 的 CI/Release，适配单 flavor（无 Kasumi / 无 full-lite-nano 矩阵）。

### 6.1 Secrets / 权限（全部沿用，不新增）

| Secret / 设置 | 用途 | 说明 |
|---|---|---|
| `RELEASE_TOKEN` | 在 `KernelSU-Modules-Repo/hybrid_mount` 创建 release | 老仓库已有 |
| `TELEGRAM_BOT_TOKEN` | TG 通知 | 老仓库已有 |
| `TELEGRAM_CHAT_ID` | TG 通知 | 老仓库已有 |
| `SIGNING_KEY` / `SIGNING_CERT` | release 签名（可选） | 老仓库已有，未配置则跳过 |
| `GITHUB_TOKEN` | 本地 release / GHCR 包读取 | 自动提供 |
| Actions 权限 | `contents: write`、`packages: read` | 老仓库已有 |

### 6.2 GitHub Actions 清单（新写，行为继承）

| Workflow | 触发 | 行为 |
|---|---|---|
| `build.yml` | push/PR 到 `rehybrid-mount`、workflow_dispatch | 构建 arm64 包 + artifact + **TG 每日/构建通知（topic 37）** |
| `release.yml` | tag `v*.*.*`、workflow_dispatch | 构建 release 包 → 校验 SHA256 → **TG release 通知（topic 6）** → 本地 GitHub Release → **KSU 模块仓库远程 Release** → sync 版本回 `rehybrid-mount` |
| `lints.yml` | push/PR | Rust fmt/clippy/test（含 Android NDK 目标）+ pnpm lint/test/build |
| `notify.yml` | tools/notify 相关变更 | notify crate fmt/clippy/test |
| `license_header.yml` | schedule | HawkEye license 检查 |
| `ci-image.yml` | 如需修改镜像再启用 | 现阶段直接复用现有 `meta-hybrid_mount-ci` 镜像 |
| `auto-label.yml` | issue 事件 | 自动打标签 / 关闭无日志 issue |
| `auto-blacklist-pr.yml` | issue 事件 | 黑名单请求自动 PR（base 改为 `rehybrid-mount`） |
| `dependency-audit.yml` | 依赖变更 | 依赖审计 |

### 6.3 CI 镜像
- 直接复用现有 `ghcr.io/hybrid-mount/meta-hybrid_mount-ci:latest`，同仓库内权限已通，无需任何配置。
- 若后续 Vue 双 UI / 依赖要求变更镜像，再在同一仓库新增 `ci-image.yml` 并更新 tag。

### 6.4 KSU 模块仓库连线（保持现状）
- 目标仓库：**`KernelSU-Modules-Repo/hybrid_mount`**（不变）。
- Release 流程与现状一致：
  1. 在本仓库 `Hybrid-Mount/meta-hybrid_mount`（ReHybrid 分支/tag）创建正式 GitHub Release。
  2. 使用现有 `RELEASE_TOKEN` 在 `KernelSU-Modules-Repo/hybrid_mount` 创建同名 tag 的 draft release。
  3. 上传 zip + `SHA256SUMS`，随后 `gh release edit --draft=false`。
- 稳定版与 pre-release 判定逻辑沿用现状：`vX.Y.0` 为稳定版，其余 patch 为 pre-release。
- 版本/tag 命名：为避免与旧历史 tag（v4.x / v5.x）冲突，ReHybrid 使用独立大版本线（建议 `v6.x.x`；最终版本号待确认）。
- `module.prop` 的 `updateJson` 在分支晋升前指向：
  ```
  https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/rehybrid-mount/update.json
  ```
  分支晋升为 `main` 后切回 `/main/update.json`。
- release 成功后同步 `update.json`、`changelog.md`、`Cargo.toml`、`Cargo.lock` 回 `rehybrid-mount`；pre-release 不写 `update.json`（沿用现状防版本号碰撞）。

### 6.5 Telegram 群组连线（保持现状）
- 保留 `tools/notify` crate，接口与现有 `xtask notify` 一致：
  - `TELEGRAM_BOT_TOKEN` + `TELEGRAM_CHAT_ID`
  - 支持 `--topic-id` 消息线程
- Topic 沿用现状：
  - **release：topic 6**
  - **dev/日常构建：topic 37**
- 文案样式沿用现有“🌾 丰收/日常耕作”通知格式，品牌名改为 **ReHybrid-Mount**。
- 本地构建无 secrets 时跳过通知并打印提示（行为与现状一致）。

### 6.6 门禁（在 `lints.yml` / 打包校验中执行）
- 全仓 `kasumi`（大小写不敏感）= 0。
- 禁止模块布局归一化逻辑（黑名单检查 `normalize_symlinked_partition_layout`、`normalize_module_layout` 等）。
- 模块目录不可变回归测试必须通过。
- License header：核心 GPL-3.0-only，WebUI Apache-2.0。
- ShellCheck：`module/**/*.sh`。

---

## 7. 与旧历史的关系及分支晋升

- 所有旧分支（`main` / `dev` / `archive/*` / `origin/*`）保持不动，作为：
  - 历史与归档；
  - v4.2.0 overlay 行为查阅；
  - 语言提交找回来源；
  - CI/Release 行为参考。
- `rehybrid-mount` 是 orphan branch，首个提交无 parent、不包含旧实现代码。
- 稳定后分支晋升（待确认）：
  1. 将旧 `main` 重命名为 `archive/legacy-main`；
  2. 将 `rehybrid-mount` 重命名为 `main` 并设为默认分支；
  3. 更新 workflow 触发分支和 `updateJson` URL 为 `/main/`。

---

## 8. 风险与对策

| 风险 | 对策 |
|---|---|
| 混合模式 magic 与 overlay 争抢路径 | planner 先决策后执行；冲突显式报错 |
| 从 0 重写导致行为回归 | 以参考行为写单元测试 + Android 实机场景清单 |
| 移除 daemon 后无实时状态 | 状态显示“上次启动快照 + shell 实时系统信息” |
| 语言提交与最终 locale 冲突 | 先 cherry-pick 原提交，再单独对齐，不 squash |
| orphan 分支上的 workflow 触发/默认分支差异 | YAML 明确触发 `rehybrid-mount`；晋升 main 时一次性替换 |
| Release tag 与旧历史 tag 冲突 | ReHybrid 使用独立大版本线（建议 `v6.x.x`） |
| `miuix-vue` / Vue 双 UI 在 WebView 中的兼容性 | 锁版本 + 真机 WebView 验证；MD3 作为回退 UI |
| 模块目录再次被归一化 | CI 黑名单 + 布局不可变回归测试双保险 |

---

## 9. 待确认清单

- [ ] orphan 分支名用 `rehybrid-mount`（或指定其他名字）
- [ ] 暂不设置新 secrets，全部沿用老仓库配置
- [ ] ReHybrid 版本线建议 `v6.x.x`（避免与旧 tag 冲突）
- [ ] 包名/模块名保持 `Hybrid Mount` + module id `hybrid_mount`
- [ ] 稳定后再执行第 7 节分支晋升（旧 main → `archive/legacy-main`，新分支 → `main`）
- [ ] 确认后从 Stage 0 开始执行
