---
name: hm-vfs
description: 维护 Hybrid Mount 的 VFS 内核子系统 hybridmount（module/vfs 内核模块 + src/vfs Rust 控制面）：改规则、CLI/doctor、LKM 选型或重编 .ko 时用。
whenToUse: 涉及 vfs 后端（planner 的 vfs 规则、hybrid-mount vfs 子命令、module/vfs 内核源码或 binaries 产物）的修改、排查与门禁时。
---

# VFS 内核子系统（hybridmount）

先读 `module/vfs/README.md`、`docs/VFS_CLI.md`、`module/vfs/src/PROVENANCE` 和 `CLAUDE.md` 的「LKM（可选）」一节；以当前文件为准。

## 构成与职责边界

- `module/vfs/src/`：fork 自 NoMount（commit 016375cd4a9e7da07b0519dd7bc492101de2a834）的内核源码。key type `hybridmount`，协议版本 `hm1`，Kconfig 符号 `HYBRIDMOUNT`，目标文件 `hybridmount.o`。GPL-2.0-only，`MODULE_LICENSE("GPL v2")`，保留 `MODULE_AUTHOR("maxsteeel")`。
- `module/vfs/binaries/`：7 个按 Android/GKI 目标命名的 `hybridmount-android<NN>-<kernel>.ko`、`list.txt`（SHA-256）、`sources.txt`（构建输入摘要）。
- `module/vfs/setup.sh`：把源码内置进内核树。`customize.sh` 只在非 arm64 安装时删除 `vfs/binaries`（源码各平台都保留）。
- `src/vfs/`：Rust 控制面 —— `backend`（keyring 绑定与 provider 选型）、`cli`、`control`（写后回读）、`doctor`、`exec`（把计划批次发给内核）、`rule`（计划树→规则）、`protocol`（字节编解码，`protocol::MAGIC` 必须与 `hybridmount.h` 的 `HYBRIDMOUNT_MAGIC_SIG` 逐字节一致，否则内核以 `-EFAULT` 拒收整页）、`lkm`/`lkm_target`（加载与候选选型）、`guard`（外来 NoMount 探测）。
- 边界：规则只发给 `hybridmount`；`nomount` key type 仅用于探测外来实现。`vfs` 修改只作用于运行态，不写配置、无 `--save`，重启后由启动配置重建。

## CLI 与 doctor（真实子命令）

设备 root shell（`docs/VFS_CLI.md` 为完整契约）：

```sh
HM=/data/adb/modules/hybrid_mount/hybrid-mount
"$HM" vfs help
"$HM" vfs doctor [--json]    # 只读：presence/version/responds/rules/uids/probe_error/list_error
"$HM" vfs version [--json]   # 内核协议版本（hm1）；顶层 version 仍是程序版本
"$HM" vfs load [--json]      # 显式选型、加载并验证随附 LKM；已有兼容 Provider 不重复加载
"$HM" vfs rule add <virtual> <real> | --whiteout <path> | --opaque <dir> [--uid UID]
"$HM" vfs rule del|list [...] | rule clear --yes
"$HM" vfs uid add|del|list [...] | uid clear --yes
"$HM" vfs clear {rules|uid|all} --yes
```

- 别名仅在 `vfs` 命名空间内：`add/a`、`del/d`、`whiteout/w`、`block/b`、`unblock/u`、`list/l`、`list uid`、`v/-v`；列表命令也接受裸 `json`。
- 退出码：0 成功，1 运行/Provider/协议错误，2 参数错误。清空必须显式目标且带 `--yes`。
- 查询与 rule/uid 操作不隐式加载 LKM；`vfs load` 之外的命令不会 insmod。手工操作后看实时状态用 `vfs doctor`，顶层 `status` 是启动快照。
- `doctor` 在 Provider 不可用时仍成功输出报告，以 `responds`、`probe_error`、`list_error` 判断设备状态。
- 批次不是事务：失败会先回读再逐项给 `confirmed`/`unconfirmed`，不自动回滚。

## 加载、熔断与规划顺序（硬性）

- 加载顺序固定为 `/data/adb/ksud insmod` → 内置 `hybrid-mount lkm-load` → 普通 `insmod`（`/system/bin/insmod`、ap/ksu busybox、PATH `insmod`），见 `src/sys/lkm.rs::INSMOD_CANDIDATES`。
- `insmod` 退出码不是成功判据：每次尝试后必须探测 key type 是否以受支持版本（`hm1`）应答，仅退出码为 0 不算成功。
- vermagic：只有本次加载的内核日志明确报告不匹配并给出期望值时，`src/sys/lkm_image.rs` 才在内存中改写 `.modinfo` 并重试一次；磁盘上的 `.ko` 永不改写。符号解析来自 `/proc/kallsyms`，期间临时放开 `kptr_restrict` 并在插入前恢复。
- 熔断：加载前写 `/data/adb/hybrid-mount/vfs_lkm_boot_guard`，尝试正常返回即删除；内核在加载期间崩溃则标记留存，下次启动跳过 VFS 而保留其他功能。要重试须先确认内核 ABI，再手动删除该文件。nuke LKM 共用同一套加载器与熔断，但保留自己的精确选型。
- 逐个候选：失败候选必须不存在或已成功卸载才试下一个；任何已存在的 Provider（含不兼容）都拒绝重复 `insmod`/卸载。
- 探测到外来 NoMount（`/proc/modules`、`/sys/module/nomount` 或 `nomount` key type 应答）时拒绝加载；模块表读不到时 fail-closed，同样不加载。
- **加载必须发生在规划之前**：`src/pipeline.rs` 在 `build_plan` 之前调用 `vfs::ensure_loaded_for_plan*`。planner 在 `vfs_available == false` 时把每条 `vfs` 规则降级为 `ignore`，执行阶段因此提前返回、走不到自己的加载分支；把加载挪到规划之后就永远加载不上。同一条约束也适用于 `vfs_strict`：致命判定在规划前完成，且仅当确有规则选择 `vfs` 时生效。

## GKI 选型（`src/vfs/lkm_target.rs`）

- 以内核 release 里的 `android<NN>` GKI 标签优先，再按内核 major/minor 匹配；同一内核线的其他打包候选依次回退。
- 不使用 Android 用户空间版本作为 VFS 的 GKI 标签，也不跨内核线替换：`5.15.197-@Coolpak@...` 与跑 Android 16 的自定义 5.15 内核都先试 `hybridmount-android13-5.15.ko`。
- 矩阵只有 7 个目标，与 `.github/workflows/kernel-module.yml` 的 DDK matrix 一一对应：android12-5.10、android13-5.10、android13-5.15、android14-5.15、android14-6.1、android15-6.6、android16-6.12。其他内核线（如 4.19、6.18）没有随附模块。
- 随附 `.ko` 仅 aarch64，非 arm64 设备只能靠内核内置 Provider。

## 重新构建内核模块与防漂移

```sh
# 本地 DDK（镜像导出 KDIR/ARCH/LLVM，仓库挂载在 /build）
ddk build --target android14-6.1 -- -C module/vfs/src

# 无 DDK：对已准备的内核树
make -C module/vfs/src KDIR=/path/to/kernel
```

- CI 覆盖全部 7 个目标：`.github/workflows/kernel-module.yml` 用 `ghcr.io/ylarod/ddk` 镜像编译，组装 `module/vfs/binaries/` 与 `list.txt`，并由真正编译的 job 写入 `sources.txt`。推送到 `dev` 且改动 `module/vfs/**` 会自动重建并提交产物；手动强制刷新：

```sh
gh workflow run 'Build VFS kernel module' --ref dev -f commit_binaries=true
```

- `sources.txt` 是 `sha256sum hybridmount.c hybridmount.h Kconfig Makefile` 再取一次 sha256 的单一摘要；`tests/shell/vfs_sources_digest.sh` 重算比对，`lints.yml` 与 `release.yml` 都会执行。`list.txt` 只证明 `.ko` 自身未变，发现不了源码已漂移。
- 编译需要 DDK，所以**源码变更与产物刷新必然是两次提交**：改源码的那次推送在该门禁上失败属预期，按分支 tip 判断，不要为让中间提交变绿而改 `sources.txt`。
- 内置集成（在干净内核树根执行）：`curl -LSs "https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/dev/module/vfs/setup.sh" | bash`，之后自行开 `CONFIG_HYBRIDMOUNT=y`（或 `=m`）；`--cleanup` 回退。脚本拒绝已集成 NoMount 的内核树。

## 本地验证

```sh
shellcheck module/*.sh module/vfs/setup.sh tests/shell/*.sh
sh tests/shell/vfs_setup.sh
sh tests/shell/vfs_sources_digest.sh
sh tests/shell/vfs_sources_digest_test.sh
(cd module/vfs/binaries && sha256sum --check list.txt)
cargo test --test vfs_cli
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check -p hybrid-mount --target aarch64-linux-android
```

- `tests/vfs_cli.rs` 在各平台断言 help 与参数错误退出码 2；非 Linux 宿主额外断言平台错误路径与旧 `vfs-doctor` JSON 兼容。`cargo test --workspace` 覆盖 `src/vfs/` 的解析、别名、分页、写后回读与 LKM 选型单测。
- 宿主机跑不了内核模块：keyring/`add_key`、`insmod`、kallsyms、`/proc/modules`、boot guard、vermagic 适配都只在 Linux/Android 分支编译，宿主机测试与交叉编译不替代设备验证。

## 汇报格式

- 逐条列出实际执行的命令与结果（PASS/FAIL），并区分三类：宿主机单测与 shell 门禁、交叉编译检查、真机验证。
- 改动 `module/vfs/src/**` 时明确说明 `sources.txt` 门禁当前会失败、重建提交是否已落地。
- 真机结论必须写明设备内核 release 与实际选中的 `.ko` 文件名（来自 `vfs doctor`）；没跑过的部分直接标未验证。
