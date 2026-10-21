# 项目约定

Hybrid Mount：Rust 核心 + Vue 3 WebUI，面向 KernelSU / APatch。
目录与挂载不变量见 `.dsh/skills/hybrid-mount-overview/SKILL.md`。

计划、排查结论默认在对话中说明；仅在用户要求时另写文档，不为流程单独创建或提交文档。
注释只解释不明显的原因、约束和安全条件，避免复述代码。

## 验证

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
node .dsh/validate-skills.mjs
node --test .github/scripts/issue-policy.test.cjs
cargo check -p hybrid-mount --target aarch64-linux-android
```

WebUI 修改在 `webui/` 运行 `pnpm lint`、`pnpm test`、`pnpm build`；依赖用 `pnpm install --frozen-lockfile`。
模块脚本修改运行 `shellcheck module/*.sh tests/shell/*.sh`。
构建入口为 `cargo xtask build [--release]`。

## Code Conventions

### Rust 代码规范

- **禁用符号**: `dbg!`, `expect()`, `todo!()`, `unimplemented!()`, `unwrap()`（workspace lint）
- **错误处理**: 使用 `Result<T>` 和 `?` 操作符，自定义 `Error` 类型在 `src/errors.rs`
- **日志**: 使用 `log` crate（`log::info!`, `log::error!` 等）；CLI 的 JSON 输出使用标准输出，不混入诊断日志
- **平台特定代码**: 用 `#[cfg(any(target_os = "linux", target_os = "android"))]` 隔离

### 代码审查要点

- 挂载与文件操作必须检查返回值
- 路径操作使用 `Path` / `PathBuf`，避免字符串拼接
- 回滚逻辑必须清理所有资源（挂载点、临时目录）
- 新增配置字段需同步更新 `module/config.toml`
- 跨模块冲突检测在 planner 阶段完成，executor 只执行

### 禁止的操作

- **禁止修改模块源目录** - 所有 staging 写入运行目录
- **禁止符号 "kasumi"** - 已移除的旧逻辑
- **禁止 `normalize_symlinked_partition_layout`** - 已移除的布局规范化

## Platform-Specific Notes

### Android 目标

- 使用 Android NDK 交叉编译
- 目标架构: `aarch64-linux-android`, `armv7-linux-androideabi`, `x86_64-linux-android`, `riscv64-linux-android`（Tier 3，无预编译 std，走 NDK r27+ 直连 + `-Z build-std`）
- `xtask` 的 Android 构建指定最低 API level 26
- 运行时需 root 权限（KernelSU 或 APatch）

### LKM（可选）

随包提供两个独立的内核模块，均由 CI 在 DDK 镜像中编译，产物连同 SHA256 清单提交回仓库：

- **ext4 sysfs nuke LKM**
  - 位置: `module/lkm/`
  - 许可证: GPL-2.0-only
  - 仅 aarch64 预编译，来源: Mountify 项目
  - 校验: `module/lkm/binaries/list.txt` 包含 SHA256
- **VFS 内核子系统（`hybridmount`）**
  - 位置: `module/vfs/`（源码在 `module/vfs/src/`）
  - 许可证: GPL-2.0-only，fork 自 NoMount
  - 仅 aarch64 预编译，按 Android/GKI 目标命名 `hybridmount-android<NN>-<kernel>.ko`
  - 校验: `module/vfs/binaries/list.txt` 包含 SHA256（由 `.github/workflows/kernel-module.yml` 生成）
  - 防漂移: `module/vfs/binaries/sources.txt` 记录构建输入（`hybridmount.c`、`hybridmount.h`、`Kconfig`、`Makefile`）的摘要，由真正编译它们的 job 写入；`tests/shell/vfs_sources_digest.sh` 重算比对，`lints.yml` 与 `release.yml` 都会执行。`kernel-module.yml` 在推送到 `dev` 且改动 `module/vfs/**` 时自动重建并提交产物，因此源码变更与产物刷新是两个提交，中间的推送会在该门禁上失败属预期

启动流程先探测内建的 key type `hybridmount`；每次启动在未响应时（不论配置是否选择 `vfs`），从 `module/vfs/binaries/` 优先选择内核 release 中 GKI 标签匹配的模块，再尝试同一内核主次版本的其他候选，加载后再探测；不使用 Android 用户空间版本作为 VFS 的 GKI 标签。加载必须发生在规划之前：planner 会把无后端可用的 `vfs` 规则降级为 `ignore`，执行器随之提前返回而走不到加载分支，等到执行阶段才加载会永远加载不上。`vfs_strict` 的失败判定同样在规划前完成，且仅当确有规则选择 `vfs` 时生效。

加载顺序为 `ksud insmod` → Rust 二进制内置的 `lkm-load` → 普通 `insmod`。`src/sys/lkm_image.rs` 负责 ELF 校验、核心符号解析和内存中的 vermagic 适配，`src/sys/lkm_compat.rs` 负责系统调用；只有本次加载的内核日志明确报告 vermagic 不匹配时才重试一次，成功后不重复加载。nuke 共用加载器，但保留原有精确选型。

### 熔断机制

两个模块共用 `src/sys/lkm.rs` 的熔断逻辑：在调用 `insmod` 前持久化熔断标记，加载尝试正常返回时移除标记。若内核在加载期间崩溃，标记会保留，下次启动跳过对应模块但保留 Hybrid Mount 其他功能。

VFS 的两个标记（`vfs_boot_guard` 规则注入、`vfs_lkm_boot_guard` 模块加载）由 `src/vfs/boot_guard.rs` 记录写入它的构建（版本 + 可执行文件大小/mtime），LKM 标记再记录候选 `.ko` 的大小/mtime。只有「同一构建写入、且候选集中仍存在同一内核对象」的标记才会阻止下一次自动尝试；其他构建留下的标记、旧版 1 字节标记、以及被新包替换掉的 `.ko` 所留下的标记都会在尝试前自动清除（`prepare_lkm_guard` / `arm_for_mutation`）。`hybrid-mount vfs guard clear --yes`（以及状态页红卡按钮）是唯一的显式清除入口，`vfs guard [--json]` 报告两个标记的判定。

## Release Process

`cargo xtask build --release` 完成：

1. 安装 WebUI 依赖并构建（`pnpm build`），输出到 `module/webroot`
2. 通过 Vite 将 `MODULE_ID` 注入 WebUI
3. 交叉编译 Rust 二进制（aarch64、armv7、x86_64、riscv64）
4. 生成 `module.prop`
5. 打包 zip（包含二进制、脚本、WebUI、LKM）

仓库根目录 `module.yaml` 提供 KernelSU Modules 展示元数据，图标为从 `icon.svg` 导出的 `icon.png`；默认分支 `main` 必须包含这些文件。首次上架流程见 `docs/KERNELSU_MODULES.md`。

`.github/workflows/release.yml` 使用固定完整 SHA 的官方 `KernelSU-Modules/tools/actions/module-release` 以 dry-run 预检 ZIP 与元数据，保留 `Hybrid-Mount-{version}-{commit_count}.zip` 原命名，按 draft → 上传 ZIP → 校验名称及 SHA256 → 发布的顺序创建不可变 release，并使用原始 ZIP 名生成 `update.json`，随后更新 `changelog.md` 和版本信息；`cargo xtask update-json` 也可单独生成更新元数据。Telegram 通知由独立的 `cargo xtask notify` 命令发送，构建命令不会自动发送。

发布构建依赖的缓存由 CI 预热：`build.yml` 在 `dev` 与 `main` 上都会运行，`main` 上那一次同时把 4 个架构的 release 产物、riscv64 的 std（Tier 3，没有可下载的 std，只能靠 `-Z build-std` 现编）与 pnpm store 写进缓存。tag run 只能读「自身 ref + 默认分支」的缓存，而 `lints.yml` 在默认分支上产出的只有 `cargo check` 产物（check 元数据推不出 release profile 的 codegen），所以这是发布能复用编译结果的唯一来源。**发布顺序因此是固定的：先合并 `dev` → `main`，等 `main` 的 Build Hybrid Mount workflow 变绿，再打 tag**；合并后立刻打 tag 不会出错，但 tag run 会抢在预热写入缓存之前启动，退回全量冷编译。

### Tag 约定

发布 tag 必须是合法 semver，**patch 不允许前导零**：`v6.2.1` 可以，`v6.2.01` 不行（Cargo 会拒绝 `version = "6.2.01"`）。预发布写成 `-<stage>.<number>`，stage 取 `alpha`、`beta`、`rc`，number 为 1–99：

```bash
cargo xtask release-version v6.2.1-rc.1   # 输出 version / version_code / prerelease
```

tag 到版本、versionCode 与预发布标志的换算只由 `xtask` 的 `ReleaseVersion` 决定，workflow 不再自带正则或 awk，避免出现「workflow 接受的 tag 被 Cargo 拒绝」这类只在构建阶段暴露的偏差。

versionCode 为 `major*100000 + minor*1000 + patch` 再乘 1000 加槽位；预发布槽位**低于**对应正式版（`v6.2.1-rc.1` = 602001301 < `v6.2.1` = 602001999），否则试过预发布的设备永远收不到正式版。新 code 一律高于历史上按整数发布的 code（如 `v6.2.0` = 602000），升级链路不受影响。

## Git Workflow

- 主分支: `main`
- 开发分支: `dev`
- PR 目标: `dev`
- 发布时从 `dev` 合并到 `main`，等 `main` 的 Build workflow 通过后再打 tag（见 Release Process：tag 只能复用默认分支已经预热好的缓存）
