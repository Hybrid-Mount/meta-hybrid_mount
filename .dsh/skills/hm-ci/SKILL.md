---
name: hm-ci
description: Hybrid Mount 的 7 个 GitHub Actions workflow 拓扑、触发条件、权限、产物提交与发布链路；改 .github/workflows、判断改动的触发面或排查 CI 失败原因时读。
whenToUse: 需要改 workflow、确认某个改动会不会触发或被门禁拦下、理解「产物刷新提交」与发布链路、排查 secrets/权限问题时。
---

# Hybrid Mount CI/CD 拓扑

内容只读自 `.github/workflows/`、`.github/dependabot.yml`、`.github/ISSUE_TEMPLATE/`、`xtask/src/main.rs` 与相关脚本。
本地跑门禁命令见 hm-verify，本地构建/发布命令见 hm-build-release；本文只回答「哪个 workflow 在什么条件下跑什么 job、会写什么、改它有什么风险」。

## Workflow 一览

| 文件 | name | 触发 | 关键 job | PR 门禁 |
| --- | --- | --- | --- | --- |
| `lints.yml` | lints-check | push/PR **全分支**（无 paths 过滤）、dispatch | rust-lints、android-target-checks（matrix 4 目标）、webui-lints | 是 |
| `build.yml` | Build Hybrid Mount | push/PR → `dev`（paths 白名单）、dispatch | build | 是 |
| `release.yml` | Release Hybrid Mount | push tag `v*.*.*`、dispatch | build → release（`needs: build`） | 否（tag 触发） |
| `kernel-module.yml` | Build VFS kernel module | push/PR → `dev`（paths `module/vfs/**`）、dispatch（input `commit_binaries`） | build-module（DDK matrix 7 目标）→ package-binaries（needs） | 是 |
| `dependency-audit.yml` | Dependency Audit | push/PR 全分支，paths 仅依赖清单 | cargo-audit、pnpm-audit | 是 |
| `auto-blacklist-pr.yml` | Auto Blacklist PR | issues `opened`/`labeled` | create-blacklist-pr | 否 |
| `auto-label.yml` | Auto Label and Close Invalid Issues | issues `opened`/`edited` | label-and-close | 否 |

「是」= 会在 PR 上运行、失败即红。哪些 check 被仓库 ruleset 标为 required 属于仓库设置，**不在仓库文件中，需确认**。

`build.yml` 的 paths 白名单：`src/**`、`Cargo.*`、`.cargo/**`、`module/**`、`webui/**`、`xtask/**`、`tools/notify/**`、`.github/workflows/build.yml`。
`dependency-audit.yml` 的 paths：`Cargo.lock`、`Cargo.toml`、`.cargo/**`、`webui/pnpm-lock.yaml`、`webui/package.json`、`xtask/Cargo.toml`、`tools/notify/Cargo.toml`、它自己。

## 会往仓库提交产物的 workflow

两处提交回写，都是「源码提交在前、产物提交在后」的两段式：

1. `kernel-module.yml` → `package-binaries`：push 到 `dev` 且改动 `module/vfs/**` 时，重建并提交 `module/vfs/binaries/`（7 个 `.ko` + `list.txt` + `sources.txt`），message `chore(vfs): refresh the prebuilt hybridmount modules [skip ci]`，`git push origin "HEAD:${GITHUB_REF_NAME}"`；`GITHUB_REF_TYPE != branch` 直接 exit 1；`git diff --cached --quiet` 时跳过。手动运行时只在 `commit_binaries=true` 才提交。
2. `release.yml` → `release` job 的 `Sync version back to dev`：`git fetch origin dev && git checkout dev`，覆盖 `update.json`、把 changelog 前插到 `changelog.md`、`sed` 版本进 `Cargo.toml` 并 `cargo update`，有 staged diff 才提交 `chore(release): sync version <tag> [skip ci]` 并 push `dev`。它落在**当时的 dev**，不是 tag 指向的 commit 上。

**两个提交之间的中间态是预期的失败**：源码提交之后、`package-binaries` 落地之前，`tests/shell/vfs_sources_digest.sh` 必然 FAIL（`lints.yml` 的 `Verify prebuilt VFS modules still match their sources`、`release.yml` 的 `Verify the prebuilt VFS modules match their sources` 都调它）。不要把它当回归修，等自动提交，或手动重建：

```bash
gh workflow run 'Build VFS kernel module' --ref dev -f commit_binaries=true
```

`license_header.yml` **已删除**：它只在每周一 `schedule` 跑一次 `korandoru/hawkeye@v6`，且只执行 `hawkeye check`（没有提交/开 PR 步骤）。它读的 `licenserc.toml` 与 `LICENCE_HEADER` 模板在 `df3dc990`（`chore(history): preserve legacy dev contributors`）的合并中丢失，因此该 job 从 2026-08-31 起连续 5 次周跑失败（`cannot load config: licenserc.toml`，exit 101）。仓库现有的 SPDX 头是手写维护的，且跨三种许可（核心 GPL-3.0-only、WebUI Apache-2.0、内核模块 GPL-2.0-only），hawkeye 一份配置只能表达一种许可文本，恢复门禁需要多份配置 + 按 `includes`/`excludes` 分次调用。
`auto-blacklist-pr.yml` 用 `GH_TOKEN=${{ github.token }}` 建分支 `chore/blacklist-<id>-<num>` 并向 `dev` 开 PR；`auto-label.yml` 只写 issue 评论/状态，不动仓库内容。

## 发布链路（tag → 版本 → update.json/changelog）

`push` tag 为 `v*.*.*` 时，`release.yml` 的 build job 里 `ReleaseVersion` 是唯一的版本事实来源（`xtask/src/main.rs`）：

1. `cargo xtask release-version "$RELEASE_TAG"` → `version` / `version_code` / `prerelease` 三行写进 `$GITHUB_OUTPUT`，同时校验 semver（非法 tag 在任何构建前就失败）。仅在 `refs/tags/` 下执行。
2. `sh tests/shell/vfs_sources_digest.sh`：tag 可指向任意 commit，所以发布前再挡一次源码/产物漂移。
3. `sed` 把 version 写进 `Cargo.toml` 并 `grep -q` 反查；`cargo xtask build --release`；上传 artifact（固定名 `package`）。
4. release job 下载 artifact，`cargo xtask notify --label "丰收 (Harvest) - <tag>" --topic-id 6`。
5. 仅 tag：`cargo xtask update-json <tag> <version_code> https://github.com/<repo>/releases/download/<tag>/<zip>`（changelog 默认指向 dev 的 `changelog.md`），再 `cp update.json /tmp/update.json`。
6. `orhun/git-cliff-action@v4`（`cliff.toml`，args `--current`，`OUTPUT=/tmp/changelog.md`）生成 changelog。
7. `softprops/action-gh-release@v3`：`draft: false`、`prerelease: ${{ env.PRERELEASE }}`、`body_path: /tmp/changelog.md`。
8. KernelSU-Modules-Repo 的远程发布步骤**已整段注释停用**（含 `secrets.RELEASE_TOKEN`）；hm-build-release 已同步标注为停用，两处描述一致，以 workflow 为准。
9. `Sync version back to dev`（见上一节）。

versionCode 换算：`(major*100000 + minor*1000 + patch) * 1000 + 槽位`；正式版槽位 `999`，预发布槽位 = stage 序号 ×100 + number，stage 固定顺序 `alpha` → `beta` → `rc`，number 取 1–99。所以 `v6.2.1-rc.1` = 602001301 **低于** `v6.2.1` = 602001999。带 `+build` 元数据、非 semver、patch 前导零、未列出的 stage 一律被 `release-version` 拒绝。
`workflow_dispatch` 触发时没有 tag：版本解析、写版本、`update-json`、建 release、同步回 dev 全部跳过，只构建 + 发通知。

## kernel-module.yml：DDK 构建与 sources.txt 门禁

- `build-module` 的 matrix：`android12-5.10`、`android13-5.10`、`android13-5.15`、`android14-5.15`、`android14-6.1`、`android15-6.6`、`android16-6.12`；`fail-fast: false`。这一步用 `docker run --rm --platform linux/amd64 -v "$PWD":/build` 跑镜像 `ghcr.io/ylarod/ddk:<target>` 的 `make -C module/vfs/src`（不是 `container:`，所以 job 本身是普通 runner）。产物改名 `dist/hybridmount-<target>.ko` 后上传 artifact。
- `package-binaries`：下载全部 artifact → `cp` 进 `module/vfs/binaries/` → `sha256sum ./*.ko | sed 's|\./||' > list.txt` → 写 `sources.txt` = `(cd module/vfs/src && sha256sum hybridmount.c hybridmount.h Kconfig Makefile) | sha256sum | cut -d' ' -f1` → `sha256sum --check list.txt` → 跑 `vfs_sources_digest.sh` → 条件提交（见上）。
- `tests/shell/vfs_sources_digest.sh` 用同一份四文件清单重算摘要比对 `sources.txt`；文件缺失时 SKIP（exit 0），只在「有产物、摘要对不上」时失败。该清单在两处显式重复（workflow 的 `sha256sum` 行与脚本里的 `BUILD_INPUTS`）：**给 `module/vfs/src` 新增编译输入必须同时改这两处**，否则新文件不进摘要，漂移不会被发现。
- `list.txt` 只证明文件自构建后未被改动，不证明与源码一致；两者是不同的门禁，别互相替代。

## 权限与 secrets 注意事项

- 只有 Telegram 通知用真实 secret：`TELEGRAM_BOT_TOKEN`、`TELEGRAM_CHAT_ID`，出现在 `build.yml`（步骤带 `if: github.event_name != 'pull_request'`，且该步骤无 `continue-on-error`）与 `release.yml`。fork PR 不提供 secrets，这个 `if` 就是防线；改动时不要去掉它，也不要把 notify 提到 PR 会跑的 job 里。
- CI 容器镜像 `ghcr.io/hybrid-mount/meta-hybrid_mount-ci:latest` 用 `container.credentials`（`username: github.actor` + `secrets.GITHUB_TOKEN`）拉取，因此相关 workflow 需要 `packages: read`。`lints.yml`、`build.yml`、`dependency-audit.yml` 在 workflow 级给了它；`kernel-module.yml` 的 workflow 级只有 `contents: read`（它不用该镜像）。
- 权限按 job 最小化：`contents: write` 只出现在会写仓库的 job——`release.yml` 的 release job、`kernel-module.yml` 的 package-binaries、`auto-blacklist-pr.yml`（另有 `pull-requests: write`、`issues: write`）、`auto-label.yml`（`issues: write`）。改 workflow 时不要把 `contents: write` 提到 workflow 级，尤其是有 `pull_request` 触发的文件。
- `[skip ci]` 保留在自动提交的 message 里：`kernel-module.yml` 的注释明确写了默认 `GITHUB_TOKEN` 推送本就不会触发 workflow，标记是为了将来换成其他推送 token 时不至于自触发循环。
- 由 `GITHUB_TOKEN` 开出的 PR 同样不会重跑 push/PR 门禁（同族平台行为，仓库内只有 push 那一处注释为证）——**需确认**是否要为此给自动黑名单 PR 换 token。

## 改 workflow 的自查清单

- [ ] 触发面：`lints.yml` 无 paths 过滤，改任何文件都会跑；`build.yml` 是显式白名单，新增顶层目录（如 `tools/<new>`、`docs/**`、`tests/**`）**不会**触发它，需要时手动加路径。
- [ ] `release.yml` 只在 tag 上做版本与发布相关步骤，新增步骤同样要用 `startsWith(github.ref, 'refs/tags/')` 兜住。
- [ ] 会提交回仓库的步骤：message 带 `[skip ci]`、确认不会自触发、确认目标 ref 是分支而非 tag。
- [ ] 权限写在 job 级、只给需要的 scope；新增 secret 前先确认 fork PR 路径不会引用它。
- [ ] paths 过滤改动是否让某个门禁「悄悄不跑」：改 `Cargo.toml`/`Cargo.lock` 才跑 audit，改 `module/vfs/src` 才重建内核模块。
- [ ] 本地能否复现（见下）；DDK、Telegram、gh release 只能靠 CI。
- [ ] 同步文档：`CLAUDE.md` 的「Release Process」「Tag 约定」「LKM（可选）」「Git Workflow」；本地命令清单同步进 `.dsh/skills/hm-verify/SKILL.md`，发布命令同步进 `hm-build-release`。
- [ ] action 版本跟仓库惯例对齐（checkout@v7、upload-artifact@v7、download-artifact@v8、cache@v6、Swatinem/rust-cache@v2、pnpm/action-setup@v6、setup-node@v7、github-script@v9、git-cliff-action@v4、action-gh-release@v3、rustsec/audit-check@v2）；`.github/dependabot.yml` 每月分组更新 github-actions，cargo/npm 走周更并锁 `target-branch: dev`。

## 本地可复现 vs 只能 CI

可本地复现（命令细节见 hm-verify）：

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
sh tests/shell/vfs_sources_digest.sh tests/shell/vfs_setup.sh tests/shell/customize_lkm.sh tests/shell/boot_lock.sh
node .dsh/validate-skills.mjs      # lints.yml: Validate DSH skills
cargo xtask release-version v6.2.1-rc.1     # 版本/versionCode/预发布判定，与 CI 同一条命令
cargo xtask build --ci                      # 等价 build.yml 的构建（需 NDK 与 pnpm）
```

只能靠 CI：`rust-lints` 的 `--cap-add SYS_ADMIN --security-opt apparmor=unconfined` 容器语义（回滚测试要私有命名空间与 tmpfs 挂载）、riscv64 的 `-Z build-std` 矩阵项（本地只能近似）、DDK 七目标的 docker 编译、Telegram 通知、`git-cliff` 与 `action-gh-release` 的发布动作、Dependabot。

## 汇报格式

- 先给结论：改动会触发哪些 workflow、哪一步会红、是否属于「产物未刷新」的预期中间态；
- 列出参与判断的文件与行号（workflow 名 + job 名），不确定的显式写「需确认」；
- 只读分析：不要为验证 CI 行为去触发 workflow。
