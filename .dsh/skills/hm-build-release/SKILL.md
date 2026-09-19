---
name: hm-build-release
description: 用 cargo xtask 构建 Hybrid Mount 模块包（WebUI + 三架构二进制 + module.prop + zip），并处理 update.json、changelog、Telegram 通知与发布流程。
whenToUse: 需要打包可刷入的 KernelSU/APatch zip、生成 update.json/changelog，或执行与复核发布流程时。
---

# 构建与发布

## 构建

```bash
cargo xtask build            # debug：WebUI + 三架构 + zip
cargo xtask build --release  # release 构建
cargo xtask build --ci       # 等价 release，CI 使用
```

`xtask` 构建流程（`xtask/src/main.rs`）：

1. pnpm build 构建 WebUI 并注入 `MODULE_ID`（输出 `module/webroot`）。
2. rustup target add（nightly）+ cargo ndk 交叉编译 aarch64 / armv7 / x86_64（min API 26）。
3. 复制 `module/` 到 `output/stage/`，二进制按后缀放入 `output/stage/binaries/hybrid-mount-<suffix>`。
4. 生成 `module.prop`（含 `metamodule=1`）。
5. 打包为 `output/Hybrid-Mount-<version>-<commit_count>.zip`。

版本来源：

- 版本 = 根 `Cargo.toml` 的 `package.version`；
- `versionCode` = (major*100000 + minor*1000 + patch) * 1000 + 槽位（正式版槽位 999，预发布见下）。

生成物（已 gitignore，勿手改/提交）：`module/module.prop`、`module/bin/`、
`module/webroot/`、`output/`。

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
cargo xtask update-json <version> <version_code> <zip_url>
```

默认 changelog URL 指向 dev 分支的 `changelog.md`，可用 `--changelog` 覆盖。

## Telegram 通知

```bash
cargo xtask notify --output output --label <label> --topic-id <id>
```

topic：6 = release，37 = dev。构建命令不会自动发通知。

## 发布流程（`.github/workflows/release.yml`）

1. `cargo xtask release-version` 解析 tag：版本、versionCode、是否预发布（tag 非法即刻失败）；
2. 把版本写入包，`cargo xtask build --release`；
3. `cargo xtask notify`；
4. 生成 changelog（git-cliff，配置见 `cliff.toml`）与发布元数据；
5. `gh release create/edit`（含 KernelSU-Modules-Repo，按预发布标志决定 `--prerelease`）；
6. 把版本同步回 `dev`。

分支模型：`main` 稳定，`dev` 开发，PR 目标为 `dev`。

## 本地发布前清单

- [ ] hm-verify 全绿
- [ ] `cargo xtask release-version <tag>` 通过，且 versionCode 高于当前线上版本
- [ ] `Cargo.toml` 版本与预期一致
- [ ] `changelog.md` 已按 `cliff.toml` 约定生成
- [ ] LKM 二进制与 `module/lkm/binaries/list.txt` 校验和一致
