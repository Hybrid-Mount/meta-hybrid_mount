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

缓存策略（tag 发布不再全量冷编译）：一次 run 只能命中「自身 ref + 默认分支」的缓存，默认分支是 `main`。`build.yml` 现在在 `dev` 与 `main` 上都跑（`on.push.branches` 加了 `main`；预热 run 不发通知，`Notify Telegram` 的 `if` 带 `github.ref != 'refs/heads/main'`），它在 `main` 上的那一次是发布唯一的预热来源——`lints.yml` 虽然也在默认分支上写缓存，但它跑的是 `cargo check`（只产元数据、debug profile），推不出 release profile 的 codegen。因此 `release.yml` 的 build job 用 `shared-key: build`（= `build.yml` 的 job id，取代自动的 job-key）复用这条释放链，`Resolve the release tag` 的 63 s 冷编译由此消除；release job 只跑 `cargo xtask notify`/`update-json` 这类宿主命令，仍旧用 `shared-key: rust-lints` 蹭 `lints.yml` 更小的宿主缓存（替掉它原来 54 s 的冷编译），不必拉 build job 那份四架构缓存。三个 workflow 都必须保留 workflow 级 `CARGO_TERM_COLOR: always`（`build.yml` 原本没有，是这次补上的），否则 key 对不上、退回前缀回退或直接 `No cache found.`。rust-cache@v2 的键是 `{prefix-key}-{shared-key 或 job}-{os}-{arch}-{sha1(rustc 版本 + 所有 CARGO/CC/CFLAGS/CXX/CMAKE/RUST* 环境变量)}-{sha1(依赖清单)}`，所以两边环境变量必须逐个一致，否则 key 永远对不上、只剩前缀回退；反过来 `Cargo.toml` 的 `package.version` 在哈希前被归一化成 `0.0.0`、`Cargo.lock` 里的 path 包被过滤，所以「写 tag 版本」的 `sed` 不影响 key。`build.yml` 另补了 pnpm store 缓存：key 与 `webui-lints` 相同，`cargo xtask build` 里的 `pnpm install --frozen-lockfile` 不再每次重下依赖。

riscv64 的 std 必须单独缓存，rust-cache 永远兜不住它：`riscv64-linux-android` 是 Tier 3、没有可下载的 std（`rustup target list --toolchain nightly` 里根本没有它），所以 `rust-toolchain.toml` 必须留 `rust-src`、`xtask/src/main.rs` 必须用 `-Z build-std=std,panic_abort`，标准库只能在 `target/riscv64-linux-android/{release,debug}` 里现场编。而 rust-cache 的 `cleanup.ts` 只保留 `cargo metadata` 里的包，`core`/`std`/`alloc`/`panic_abort`/`compiler_builtins` 都不在其中，于是每次都在写归档前把 sysroot 从 `deps/` 与 `.fingerprint/` 删掉——**即使日志显示 `full match: true` 也照删**（run 36726042842 实测：缓存完全命中后仍重编 core/std 约 58 s，而 aarch64/armv7/x86_64 与 workspace crate 全部复用）。`cache-targets`/`cache-all-crates`/`cache-directories` 都改不了这个行为：清理只走 workspace target 目录与 `CARGO_HOME`，路径只要在 `target/` 之内就一定会被剪掉。因此 `build.yml` 用独立的 `actions/cache/restore@v6` + `actions/cache/save@v6` 直接缓存 `target/riscv64-linux-android`，key = `riscv64-sysroot-${runner.os}-${rustc -vV 经 sha256 取前 16 位}-${hashFiles('Cargo.lock', 'xtask/src/main.rs')}`（用 `restore-keys` 前缀回退）。key 里的 toolchain 身份片段是必需的：`channel = "nightly"` 不锁日期，rustc 每晚都在变，而缓存条目一经写入不可覆盖，key 里不带 rustc 版本就会永久命中一个过期的 sysroot。两条缓存步骤都带 `continue-on-error: true`，缓存出问题不能挡住构建；save 只在非 PR 上执行（`steps.riscv64-sysroot.outputs.cache-primary-key != ''` 同时保证 restore 步骤没跑成时不 save）。`release.yml` 的 build job 也插入了同样的 `actions/cache/restore@v6`（key 与 `build.yml` 逐字一致，`continue-on-error: true`）但**故意不写 save**：在 tag ref 上写进去只有重跑同一个 run 读得到，纯占预算；它同时补了 pnpm store 缓存步骤（key 与 `build.yml`/`webui-lints` 同形状），因为 `cargo xtask build` 里的 `pnpm install --frozen-lockfile` 之前每次发布都重下依赖。发布顺序因此固定：先合并 `dev` → `main`，等 `main` 的 Build run 变绿再打 tag；顺序反了不会构建出错，但 tag run 会抢在预热写缓存之前启动，退回全量冷编译。

## kernel-module.yml：DDK 构建与 sources.txt 门禁

- `build-module` 的 matrix：`android12-5.10`、`android13-5.10`、`android13-5.15`、`android14-5.15`、`android14-6.1`、`android15-6.6`、`android16-6.12`；`fail-fast: false`。这一步用 `docker run --rm --platform linux/amd64 -v "$PWD":/build` 跑镜像 `ghcr.io/ylarod/ddk:<target>` 的 `make -C module/vfs/src`（不是 `container:`，所以 job 本身是普通 runner）。产物改名 `dist/hybridmount-<target>.ko` 后上传 artifact。
- `package-binaries`：下载全部 artifact → `cp` 进 `module/vfs/binaries/` → `sha256sum ./*.ko | sed 's|\./||' > list.txt` → 写 `sources.txt` = `(cd module/vfs/src && sha256sum hybridmount.c hybridmount.h Kconfig Makefile) | sha256sum | cut -d' ' -f1` → `sha256sum --check list.txt` → 跑 `vfs_sources_digest.sh` → 条件提交（见上）。
- `tests/shell/vfs_sources_digest.sh` 用同一份四文件清单重算摘要比对 `sources.txt`；文件缺失时 SKIP（exit 0），只在「有产物、摘要对不上」时失败。该清单在两处显式重复（workflow 的 `sha256sum` 行与脚本里的 `BUILD_INPUTS`）：**给 `module/vfs/src` 新增编译输入必须同时改这两处**，否则新文件不进摘要，漂移不会被发现。
- `list.txt` 只证明文件自构建后未被改动，不证明与源码一致；两者是不同的门禁，别互相替代。

## 权限与 secrets 注意事项

- 只有 Telegram 通知用真实 secret：`TELEGRAM_BOT_TOKEN`、`TELEGRAM_CHAT_ID`，出现在 `build.yml`（步骤带 `if: github.event_name != 'pull_request' && github.ref != 'refs/heads/main'`，后半段只挡 `main` 的预热 run，且该步骤无 `continue-on-error`）与 `release.yml`。fork PR 不提供 secrets，`github.event_name != 'pull_request'` 就是防线；改动时不要去掉它，也不要把 notify 提到 PR 会跑的 job 里。
- CI 容器镜像 `ghcr.io/hybrid-mount/meta-hybrid_mount-ci:latest` 用 `container.credentials`（`username: github.actor` + `secrets.GITHUB_TOKEN`）拉取，因此相关 workflow 需要 `packages: read`。`lints.yml`、`build.yml`、`dependency-audit.yml` 在 workflow 级给了它；`kernel-module.yml` 的 workflow 级只有 `contents: read`（它不用该镜像）。
- 权限按 job 最小化：`contents: write` 只出现在会写仓库的 job——`release.yml` 的 release job、`kernel-module.yml` 的 package-binaries、`auto-blacklist-pr.yml`（另有 `pull-requests: write`、`issues: write`）、`auto-label.yml`（`issues: write`）。改 workflow 时不要把 `contents: write` 提到 workflow 级，尤其是有 `pull_request` 触发的文件。
- `[skip ci]` 保留在自动提交的 message 里：`kernel-module.yml` 的注释明确写了默认 `GITHUB_TOKEN` 推送本就不会触发 workflow，标记是为了将来换成其他推送 token 时不至于自触发循环。
- 由 `GITHUB_TOKEN` 开出的 PR 同样不会重跑 push/PR 门禁（同族平台行为，仓库内只有 push 那一处注释为证）——**需确认**是否要为此给自动黑名单 PR 换 token。

## 改 workflow 的自查清单

- [ ] 触发面：`lints.yml` 无 paths 过滤，改任何文件都会跑；`build.yml` 是显式白名单，新增顶层目录（如 `tools/<new>`、`docs/**`、`tests/**`）**不会**触发它，需要时手动加路径。`build.yml` 同时在 `dev` 与 `main` 上跑，`main` 那一次是发布的缓存预热（见缓存策略一节），动 `on.push.branches` 前先确认释放链还接得上。
- [ ] `release.yml` 只在 tag 上做版本与发布相关步骤，新增步骤同样要用 `startsWith(github.ref, 'refs/tags/')` 兜住。
- [ ] 会提交回仓库的步骤：message 带 `[skip ci]`、确认不会自触发、确认目标 ref 是分支而非 tag。
- [ ] 权限写在 job 级、只给需要的 scope；新增 secret 前先确认 fork PR 路径不会引用它。
- [ ] paths 过滤改动是否让某个门禁「悄悄不跑」：改 `Cargo.toml`/`Cargo.lock` 才跑 audit，改 `module/vfs/src` 才重建内核模块。
- [ ] 本地能否复现（见下）；DDK、Telegram、gh release 只能靠 CI。
- [ ] 同步文档：`CLAUDE.md` 的「Release Process」「Tag 约定」「LKM（可选）」「Git Workflow」；本地命令清单同步进 `.dsh/skills/hm-verify/SKILL.md`，发布命令同步进 `hm-build-release`。
- [ ] action 版本跟仓库惯例对齐（checkout@v7、upload-artifact@v7、download-artifact@v8、cache@v6、Swatinem/rust-cache@v2、pnpm/action-setup@v6、setup-node@v7、github-script@v9、git-cliff-action@v4、action-gh-release@v3、rustsec/audit-check@v2）；`.github/dependabot.yml` 每月分组更新 github-actions，cargo/npm 走周更并锁 `target-branch: dev`。
- [ ] 缓存契约别拆散，它现在是两条链：**释放链** = `build.yml` 在默认分支 `main` 上产出的 release 缓存（job id `build`）+ `release.yml` build job 的 `shared-key: build`；**宿主链** = `lints.yml` 的 `rust-lints` + `release.yml` release job 的 `shared-key: rust-lints`。两条链都靠 workflow 级 `CARGO_TERM_COLOR: always` 对齐（`build.yml`/`release.yml`/`lints.yml` 三处都要有），少写一处或给 `rust-lints` 加 `shared-key`，对应 job 就退回全量冷编译（表现为 `No cache found.`）。`release.yml` build job 的 pnpm store 与 riscv64 sysroot 步骤也必须与 `build.yml`/`webui-lints` 保持同一 key 形状；`build.yml` 的 `main` 触发是释放链的上游，删掉它 tag 就只能读 `lints.yml` 的 `cargo check` 产物。PR 与 tag 上写入缓存要有意识：仓库 10 GB 缓存预算已满，PR 缓存从不复用，所以 rust-cache 一律带 `save-if: ${{ github.event_name != 'pull_request' }}`。
- [ ] tag 上的缓存一律 restore-only：`release.yml` 两个 job 的 rust-cache 步骤都带 `save-if: "false"`，而 rust-cache 的 `save.ts` 在 `save-if !== "true"` 时**直接 return**（连 cleanup 都不执行），所以 tag 运行只读不写。理由是 tag ref 不可变且一次发布只跑一次：写进去的条目只有「重跑同一个 run」能读到，却要占共享预算（实测 3 个 tag × 304 MB 全是没人复用的死条目）；要恢复可写只需删掉 `save-if`，但先想清楚谁会读它。反之 `build.yml` 的 riscv64 sysroot 缓存是有意写 `dev`/`main` 的（`main` 那次才是发布要读的）：它的 save 步骤带 `if: github.event_name != 'pull_request' && steps.riscv64-sysroot.outputs.cache-primary-key != ''`，别误删这个 if。

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
