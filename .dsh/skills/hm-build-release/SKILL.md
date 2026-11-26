---
name: hm-build-release
description: 用 cargo xtask 构建 Hybrid Mount 模块包（普通版四架构与 Wear 三架构 + module.prop + 双 ZIP），并处理 update.json、changelog、Telegram 通知与发布流程。
whenToUse: 需要打包可刷入的 KernelSU/APatch zip、生成 update.json/changelog，或执行与复核发布流程时。
---

# 构建与发布

## 构建

```bash
cargo xtask build            # debug：普通版 + Wear 双包
cargo xtask build --release  # release 构建
cargo xtask build --ci       # 等价 release，CI 使用
cargo xtask build --release --flavor wear  # 仅 Wear（normal 为仅普通版）
```

`xtask` 构建流程（`xtask/src/main.rs`）：

1. 按 flavor 注入 `MODULE_ID=hybrid_mount`、`WEBUI_FLAVOR=normal|wear`，构建到 `output/webui-<flavor>/`。
2. 普通版编译 arm64 / armv7 / x86_64（API 26）及 riscv64（API 35，直接 NDK + build-std）；Wear 编译 arm64 / armv7 / x86_64 三个目标。
3. 复制 `module/` 到 `output/stage-<flavor>/`，清除继承的 webroot/binaries，复制所选前端与架构，保留内核资产和来源验证。
4. 生成 `module.prop`：共用 ID、版本与 versionCode，`packageFlavor` 和 `updateJson` 按包区分。
5. 生成 `output/Hybrid-Mount-<version>-<commit_count>.zip`、`output/Hybrid-Mount_Wear-<version>-<commit_count>.zip`。
6. CI 用 `.github/scripts/package_artifacts.py` 验证恰好两个包、架构、更新渠道、版本、来源修订和哈希。不得只取第一个 ZIP。

版本来源：

- 版本 = 根 `Cargo.toml` 的 `package.version`；
- `versionCode` = (major*100000 + minor*1000 + patch) * 1000 + 槽位（正式版槽位 999，预发布见下）。

生成物（已 gitignore，勿手改/提交）：`output/`（前端、staging 与双 ZIP）。

外部依赖：cargo-ndk、Android NDK、pnpm。

## Tag 约定与 versionCode

发布 tag 必须是合法 semver，**patch 不允许前导零**：`v6.2.1` 可以，`v6.2.01` 不行——
Cargo 会拒绝 `version = "6.2.01"`，而 tag 已经推上去了。预发布写成 `-<stage>.<number>`，
stage 取 `alpha` / `beta` / `rc`，number 为 1–99。

```bash
cargo xtask release-version v6.2.1        # version=6.2.1        version_code=602001999 prerelease=false
cargo xtask release-version v6.2.1-rc.1   # version=6.2.1-rc.1   version_code=602001301 prerelease=true
```

打 tag 前先跑一次：它同时校验 semver、算出 versionCode、判断是否预发布。workflow 用的
就是这条命令，所以本地通过 = CI 通过。

versionCode 的预发布槽位**低于**对应正式版（`rc.1` = …301 < 正式版 = …999）：设备只在
code 更高时才收到更新，若预发布高于正式版，试过预发布的设备将永远收不到正式版。新 code
一律高于历史上按整数发布的 code（`v6.2.0` = 602000），因此老设备升级链路不受影响。

## update.json

```bash
cargo xtask update-json <version> <version_code> <normal_zip_url>
cargo xtask update-json <version> <version_code> <wear_zip_url> --flavor wear
```

分别写 `update.json`、`update-wear.json`，避免更新切换 flavor。默认 changelog URL 指向 dev 分支的 `changelog.md`，可用 `--changelog` 覆盖。

## Telegram 通知

```bash
cargo xtask notify --output output --label <label> --topic-id <id>
```

topic：6 = release，37 = dev。构建命令不会自动发通知。

## 发布流程（`.github/workflows/release.yml`）

1. `cargo xtask release-version` 校验 tag 并统一生成版本事实；验证 VFS 产物来源。
2. 写版本后 `cargo xtask build --release`，验证双 ZIP，上传同一个 artifact。
3. release job 下载双包、验证身份和哈希，生成 changelog，分别执行官方 dry-run precheck。
4. 上传两个原文件名到同一个 draft；`.github/scripts/release-assets.cjs` 确认恰好两个附件且 SHA256 一致，再发布并检查 immutable。
5. 分别生成两个 update JSON，同步到 dev（连同版本与 changelog）。
6. 将两个包一起交给预编译 `hybrid-mount-notify`，topic 6；每个附件注明普通版或 Wear 与架构。日常构建 topic 37，同样双包。

目录只匹配 `Hybrid-Mount-*.zip`；Wear 使用下划线分隔，避免目录误选。官方匹配器仅支持 `*` 和 `?`，不要用字符类或否定 glob。

分支模型：`main` 稳定，`dev` 开发，PR 目标为 `dev`。

## 本地发布前清单

- [ ] hm-verify 全绿
- [ ] `cargo xtask release-version <tag>` 通过，且 versionCode 高于当前线上版本
- [ ] `Cargo.toml` 版本与预期一致
- [ ] `changelog.md` 已按 `cliff.toml` 约定生成
- [ ] LKM 二进制与 `module/lkm/binaries/list.txt` 校验和一致
