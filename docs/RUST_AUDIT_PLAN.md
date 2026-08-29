# Hybrid Mount Rust 全面审计与改进计划

> - 状态：待执行
> - 基线分支：`dev`
> - 基线提交：`ba06309d`
> - 编制日期：2026-08-29
> - 复核日期：2026-08-29（基线全部复验通过；缺口补遗见 §22）
> - 适用范围：Rust 主程序、`xtask`、通知工具、Android/Linux 构建与设备验证

## 1. 目标

本计划用于把当前 Rust 审查发现转化为可分批实施、可验证、可回滚的工程任务。核心目标如下：

1. 确保任何启动阶段失败都不会留下未追踪挂载、临时目录或部分提交状态。
2. 配置损坏、模块身份冲突、LKM 不匹配等高风险输入必须明确失败，不能静默切换到危险默认值。
3. OverlayFS、Magic Mount、ext4 staging、KernelSU/APatch 路径共享同一套生命周期和错误语义。
4. 复用项目已有安全实现，减少重复代码、直接系统调用和不必要依赖。
5. 把主机单元测试、Linux/Android 编译检查和真机验证明确分层，避免过度声明验证范围。
6. 保持现有稳定路径、CLI、配置和 WebUI 协议兼容，不进行无收益的品牌式重命名或大拆 crate。

## 2. 非目标

以下内容不属于本轮计划：

- 不重写 OverlayFS 或 Magic Mount 的核心算法。
- 不更换 KernelSU/APatch 对接协议。
- 不移动或改名稳定路径，例如 `config.toml`、`scan.ret`、`run/state.json`、`modules.img`。
- 不把当前三个 workspace member 拆成大量微型 crate。
- 不以 Windows 测试结果代替 Android/Linux 或真机结论。
- 不在没有设备回退方案时一次性合并全部高风险改动。

## 3. 必须保持的系统不变量

后续每个 PR 都必须证明没有破坏下列不变量：

- `/data/adb/modules/<id>/system/**` 始终是只读输入，扫描、规划和执行阶段不得修改源模块。
- 同一个实际文件目标只能归属一个后端；普通结构目录可以被 OverlayFS 和 Magic Mount 共享。
- 失败路径的清理顺序与创建顺序相反，子挂载必须先于父挂载卸载。
- `state.json` 和 `active_mounts` 表示最终状态，不能只反映中间阶段的乐观计数。
- 配置文件缺失与配置文件损坏是两种不同状态。
- LKM 文件名或内核版本匹配只代表候选选择，不代表 ABI 已验证。
- `disable_umount = true` 是唯一允许主动保留对应临时挂载资源的配置语义。
- CLI JSON 字段和稳定路径属于脚本与 WebUI 的兼容接口。
- KernelSU try-umount 注册成功不等于挂载已经成功，也不等于实际卸载已经发生。

## 4. 当前验证基线

编制计划时，当前代码已通过：

- `cargo fmt --all -- --check`
- `cargo metadata --locked --no-deps`
- `cargo test --workspace --all-targets --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- stable Rust 1.97 的 workspace 测试
- `x86_64-unknown-linux-gnu` 主程序检查
- `aarch64-linux-android` 主程序检查

现有 Rust 测试总计 110 项通过，但该结果不覆盖真实 mount namespace、loop device、SELinux、KernelSU/APatch、LKM ABI 或 Android 启动时序。

> PR4 实施后：workspace 测试通过（host 113+3+1=117；Linux 另含 1 个 cfg(unix) 符号链接测试），stable 1.97 与两个 Linux/Android 目标 check 通过。
>
> PR5 实施后：workspace 测试通过（host 124+3+1=128；Linux 另含 1 个 cfg(unix) 符号链接测试），stable 1.97 与 x86_64-linux-gnu/aarch64-linux-android check 通过。
>
> PR6/7 实施后（基于 ext4 块大小修复 `bd2ae521`）：workspace 测试通过（host 138+3+1=142；Linux 另含 cfg(unix) 与 mountinfo 测试），stable 1.97 与 x86_64-linux-gnu/aarch64-linux-android check 通过。
>
> PR8 实施后：workspace 测试通过（host 147+3+1=151），fmt/clippy（host 与 Linux target all-targets）及 x86_64-linux-gnu/aarch64-linux-android check 通过；Linux mount namespace 故障注入测试已加入并在 Linux CI 执行。
>
> PR9 实施后（本地边界）：workspace 测试通过（host 147+3+1=151）；Magic Linux-only 测试通过目标编译与 Clippy，x86_64 Linux 及 aarch64/armv7/x86_64 Android check 通过。本机没有 Linux 运行环境，因此 Linux 测试实际执行以推送后 CI 为准，Android 真机候选测试仍留作发布门禁。
>
> PR11 实施后（本地边界）：workspace 测试通过（host 165+3+1=169），fmt/clippy（host 与 Linux target all-targets）及 x86_64-linux-gnu/aarch64-linux-android/armv7-linux-androideabi/x86_64-linux-android check 通过；`e2fsck`/`mke2fs`/`getprop`/`insmod`/`ksud`/`apd` 已迁入统一 runner，Linux runtime CI 与 Android 真机边界仍待推送后验证。

### 4.1 验证证据等级

| 等级 | 证据 | 可以证明 | 不能证明 |
| --- | --- | --- | --- |
| L0 | 格式、metadata、静态检查 | manifest 与基础语法一致 | 运行正确 |
| L1 | Windows/主机单元测试 | 纯逻辑与序列化行为 | Linux syscall 和真机行为 |
| L2 | Linux/Android 交叉编译与 Clippy | 目标代码可编译、cfg 基本正确 | mount/SELinux/LKM 可用 |
| L3 | Linux namespace 集成测试 | 挂载、回滚、失败注入行为 | Android root 框架兼容 |
| L4 | Android 真机矩阵 | KernelSU/APatch、SELinux、启动时序 | 未覆盖设备与内核组合 |

## 5. 审计范围与责任图

| 区域 | 主要文件 | 风险重点 |
| --- | --- | --- |
| 配置与 patch | `src/config.rs` | 损坏配置回退、原子写、三态字段、协议兼容 |
| 模块扫描 | `src/scanner.rs` | 模块 ID、重复模块、符号链接、读取失败 |
| 共享树与规划 | `src/mount_tree.rs`、`src/plan/` | 后端冲突、优先级、确定性排序 |
| 流水线 | `src/pipeline.rs` | 跨阶段回滚、提交边界、状态落盘 |
| OverlayFS | `src/overlayfs/` | 子挂载、64 层分段、shallow layer、卸载顺序 |
| Magic Mount | `src/magic_mount/` | tmpfs、bind/move、只读重挂载、错误吞没 |
| 存储 | `src/storage/` | ext4 镜像、loop 生命周期、空间计算 |
| LKM 与 sysfs nuke | `src/sys/nuke.rs` | ABI 候选、哈希、熔断、危险 override |
| 系统辅助 | `src/sys/`、`src/utils/` | xattr、所有权、临时目录、mountinfo |
| 状态与 CLI | `src/state.rs`、`src/cli.rs` | 最终状态真实性、稳定 JSON 契约 |
| 构建与发布 | `xtask/` | 工具链固定、命令错误、ZIP 可重复性 |
| 通知工具 | `tools/notify/` | Tokio 特性、网络错误边界、凭据泄漏 |

## 6. 优先级定义

- **P0：启动安全或数据一致性问题。** 可能导致残留挂载、错误后端、bootloop 或高风险 LKM 加载。
- **P1：可靠性与可诊断性问题。** 会扩大故障影响、隐藏根因或使状态不可信。
- **P2：依赖、组织与构建效率问题。** 不直接改变挂载安全，但能降低维护成本。
- **P3：可选优化。** 只有在前面阶段稳定后才实施。

## 7. 阶段 0：冻结基线与复现环境

### 7.1 仓库基线

- [ ] 执行前获取并比较当前 `origin/dev`，记录基线提交。
- [ ] 保存 `cargo metadata --locked` 和 `cargo tree --workspace --locked` 输出摘要。
- [ ] 记录现有未跟踪诊断目录，所有后续操作不得覆盖或清理用户文件。
- [ ] 记录当前 release ZIP 内容、文件权限和 SHA-256，作为发布兼容基线。
- [ ] 记录 `show-config`、`modules`、`status`、`install-state` 的 JSON 样例。

### 7.2 工具链策略

- [ ] 确认项目是否真正使用 nightly-only 能力。
- [ ] 若 stable 1.97 持续通过，评估把 `rust-toolchain.toml` 从浮动 `nightly` 改为明确 stable 版本。
- [ ] 若必须保留 nightly，固定到日期版本，避免每日漂移。
- [ ] 在 `Cargo.toml` 中声明可支持的 `rust-version`。
- [ ] 让 `xtask` 使用仓库工具链配置，不再硬编码 `+nightly` 和 `--toolchain nightly`。

### 7.3 阶段验收

- [ ] 基线命令可在干净 checkout 中重复。
- [ ] CI 与本地命令使用相同 `--locked` 策略。
- [ ] 工具链变化不会改变发布 ZIP 内容或运行时行为。

## 8. 阶段 1：P0 配置安全与持久化

> 实施状态：PR4 已实现并通过本地全部门禁（host 117 项测试），已提交。

### 8.1 区分缺失、损坏和 I/O 故障

当前 `Config::load_or_default` 对读取失败和解析失败统一回退默认 Overlay。计划改为：

- [x] 文件不存在：允许使用默认配置，并记录明确的 `config_missing` 状态。
- [x] 文件存在但 TOML 无效：拒绝执行挂载流水线，保留可查询的错误状态（`show-config` 返回带路径上下文的错误；线格式 JSON 错误留待 PR12）。
- [x] 权限、短读或其他 I/O 错误：拒绝执行挂载流水线，不得伪装成配置缺失。
- [x] `show-config` 能返回结构化错误，WebUI 可以区分“未创建配置”和“配置损坏”（缺失 → 成功输出默认配置 + `config_missing: true`；损坏 → 非零退出 + 错误信息）。
- [x] 若需要兼容恢复模式，设计显式命令或显式配置开关，不使用隐式回退（现有 `gen-config` 即显式重置命令）。

### 8.2 复用原子写入

- [x] `Config::save` 复用 `src/sys/fs.rs::atomic_write`。
- [x] 保证临时文件与目标文件在同一目录，写入后同步文件和父目录（父目录 fsync 失败按保存失败处理，见 G07）。
- [x] 失败时删除临时文件；遗留临时文件不得被配置加载器误识别。
- [x] 测试原文件存在、原文件不存在、写入失败和 rename 失败。

### 8.3 配置协议兼容

- [x] 为默认配置和 patch 配置建立 JSON/TOML snapshot（默认 TOML/JSON golden + 既有 patch 合并/三态测试）。
- [x] 保持 `Option<Option<Mode>>` 的 absent/null/value 三态语义。
- [x] 当前只有一个三态字段时保留项目内 helper；出现多个字段后再评估 `serde_with::rust::double_option`。
- [x] 未知字段继续明确报错，但错误必须包含字段与配置路径上下文。

### 8.4 阶段验收

- [x] 损坏配置绝不会静默切到 Overlay。
- [x] 保存期间进程被终止不会留下截断的正式配置。
- [x] CLI/WebUI 能展示配置错误而不是只显示默认值。

## 9. 阶段 2：P0 模块身份与扫描不变量

> 实施状态：PR5 已实现并通过本地全部门禁（host 128 项测试），已提交。

### 9.1 `ModuleId` 强类型

- [x] 新建 `ModuleId` newtype，以 `TryFrom<String>` 集中执行合法性验证。
- [x] 支持 `Display`、`AsRef<str>`、排序、哈希以及必要的 serde map-key 行为。
- [x] scanner、config rules、mount tree、plan、state 不再传递未验证的模块 ID `String`。
- [x] 不为 ASCII ID 规则引入 `regex`。

### 9.2 扫描一致性

- [x] 校验目录名与 `module.prop` 中的 ID 一致。
- [x] 检测不同目录声明同一 ID；不得静默排序后继续。
- [x] 对缺失、目录型、符号链接型 `module.prop` 保持“辅助目录跳过”语义。
- [x] 对真正模块的无效 ID、重复 ID、必填字段缺失给出结构化诊断。
- [x] 明确每类 `read_dir`、metadata、文件读取错误是跳过、警告还是致命。
- [x] 不跟随分区根符号链接，避免重复扫描和 staging 膨胀。

### 9.3 测试矩阵

- [x] 有效 ID、非法首字符、非法字符、空 ID。
- [x] 目录名与 ID 不一致。
- [x] 两个目录声明相同 ID。
- [x] 普通辅助目录、符号链接 `module.prop`、目录型 `module.prop`。
- [x] `product -> system/product` 等分区别名不产生重复节点。
- [x] `.replace`、whiteout、符号链接和特殊文件保持既有语义。

### 9.4 阶段验收

- [x] 无法构造未验证 `ModuleId`。
- [x] scanner、plan、state 对同一模块集合得出一致结果。
- [x] 重复或冲突模块不会覆盖彼此 staging 路径。

## 10. 阶段 3：P0 挂载事务与全流水线回滚

### 10.1 统一挂载事务

设计项目内 `MountTransaction` 或 `MountJournal`，不直接使用通用 `scopeguard`：

> 实施状态：PR8 已实现全流水线事务接入、故障注入与基线校验，已提交。

- [x] 每个成功副作用立即登记对应清理动作。
- [x] 清理动作按逆序执行。
- [x] 支持显式 `commit`、`disarm` 和 `rollback`。
- [x] `Drop` 只做最后防线并记录失败；正常路径显式调用可返回错误的清理。
- [x] 回滚结果包含每个失败目标，不能只保留最后一个错误。
- [x] `disable_umount` 只解除明确允许保留的动作，不能整体关闭错误清理。

计划纳入统一事务的资源：

- Overlay storage handle
- runtime temp directory
- Magic staging tmpfs
- Overlay intermediate staging mounts
- bind/move mount 目标
- loop device 与 ext4 mount
- KernelSU try-umount 待提交列表

### 10.2 扩大回滚边界

- [x] Overlay 成功后，Magic、KernelSU 注册、状态保存或最终清理失败时仍能回滚 Overlay。
- [x] 只有所有后端、状态写入和必要提交完成后，事务才进入 committed 状态。
- [x] 状态保存失败不能留下“挂载成功但状态失败”的无主状态。
- [x] 若产品决定保留已成功挂载，必须把它定义为显式策略并写入状态，不能由代码路径偶然决定。

### 10.3 `is_mounted` 错误语义

- [x] 把 `is_mounted(path) -> bool` 改为 `Result<bool>`。
- [x] `/proc/self/mountinfo` 读取失败必须向上传播。
- [x] 仅将明确的“不存在/不是挂载点”映射为 `false`。
- [x] 最终状态确认和回滚都使用同一 mountinfo 快照或同一查询层。

### 10.4 卸载策略

- [x] 子挂载按路径深度从深到浅卸载。
- [x] 普通卸载失败时，仅在策略允许时使用 `MNT_DETACH`。
- [x] 对 `EBUSY`、`EINVAL`、目标消失分别处理并记录。
- [x] 不再只卸载根目录后假设所有子挂载已经消失。
- [x] 回滚结束后重新读取 mountinfo，确认本事务创建的目标全部消失。

### 10.5 故障注入点

- [x] Overlay 第 N 个目标挂载失败。
- [x] Overlay 全部成功后 Magic 第一个目标失败。
- [x] Magic 成功后 KernelSU 注册失败。
- [x] 状态 JSON 原子 rename 失败。
- [x] mountinfo 读取失败。
- [x] 子挂载卸载返回 `EBUSY`。
- [x] staging 目录删除失败。

### 10.6 阶段验收

- [x] 任一注入点失败后，最终 mountinfo 与执行前一致。
- [x] 回滚失败包含目标、动作、errno 和原始阶段。
- [x] 不存在“日志显示失败但仍保留未登记挂载”的路径。

## 11. 阶段 4：P0/P1 后端执行器审计

### 11.1 OverlayFS

- [ ] 验证 64 lowerdir 分段边界：0、1、63、64、65、128 层。
- [ ] 检查 shallow layer 与中间 open-tree/move-mount 生命周期。
- [ ] 验证 `.replace` 转换为 opaque xattr，whiteout 保持设备节点语义。
- [ ] 验证符号链接不被跟随，所有权和 SELinux 上下文复制失败有明确策略。
- [ ] 中间 staging mount 全部登记到统一挂载事务。
- [ ] 根目标卸载前先处理所有子挂载。

### 11.2 Magic Mount

- [x] 补齐 executor 成功、失败和回滚测试。
- [x] 直接子节点失败必须导致该阶段失败，不能在“没有 tmpfs”分支静默继续。
- [x] 只读重挂载失败不能只发 warning 后报告整体成功；按目标撤销刚创建的 bind/move staging 后返回 `Err`。
- [x] bind、move、symlink、whiteout、`.replace` 各自返回结构化结果。
- [x] `active_mounts` 只记录真实 mount target，不把普通 symlink 操作伪装成挂载。
- [x] Magic 执行失败后回滚本阶段以及此前已登记到事务的 Overlay 动作（PR8 事务 + PR9 executor 失败传播）。

### 11.3 ext4 storage 与 loop device

- [ ] 检查镜像空间估算对硬链接、稀疏文件、特殊文件和符号链接的处理。
- [ ] 明确 `e2fsck` 退出码 0..=3 的既有兼容语义并建立测试。
- [ ] loop attach、mount、resize、repair、detach 每一步都登记生命周期动作。
- [ ] `EBUSY` 重试必须有上限、退避和最终诊断。
- [ ] ext4 与 tmpfs 两种 storage mode 使用相同的上层事务接口。

### 11.4 阶段验收

- [ ] Overlay 与 Magic 的失败语义一致：失败就是 `Err`，降级必须显式记录策略。
- [ ] 每个成功目标都能追溯到 plan source 和最终 mountinfo。
- [ ] 后端执行器具备不需要 root 的 fake/fault-injection 测试。

## 12. 阶段 5：P0 LKM 选择、完整性与熔断

### 12.1 候选选择

- [ ] 把内核主次版本、Android/GKI 标签、架构映射成显式支持矩阵。
- [ ] 未知组合直接拒绝自动加载。
- [ ] 文档和日志统一使用“候选匹配”，不能宣称“ABI 已兼容”。
- [ ] 任意路径环境变量 override 必须改为受限的、可审计的显式调试选项。
- [ ] override 路径需要 canonicalize，并限制在允许目录或要求额外确认标记。

### 12.2 运行时完整性

- [ ] 构建时生成随二进制编译的 LKM SHA-256 manifest。
- [ ] `insmod` 前读取目标文件并验证哈希。
- [ ] 哈希缺失、不匹配、文件不是普通文件或路径被替换时拒绝加载。
- [ ] 评估使用小型 `sha2` 依赖；不能依赖外部 `sha256sum` 命令。
- [ ] 发布阶段验证 ZIP 中 LKM 与 manifest 一致。

### 12.3 熔断与恢复

- [ ] 保持 `create_new` boot-guard，前次未完成时拒绝自动重试。
- [ ] guard 内容包含候选标识、哈希、内核 release 和 mount target。
- [ ] 成功与失败路径明确何时删除或保留 guard。
- [ ] 提供人工恢复说明，不自动重复危险写操作。

### 12.4 阶段验收

- [ ] 修改 `.ko` 任意字节后加载被阻止。
- [ ] 未支持内核组合不会执行 `insmod`。
- [ ] 模拟崩溃后第二次启动被熔断，但 Hybrid Mount 其他能力仍可诊断。
- [ ] 至少在每个声明支持的真机内核组合上完成一次 ABI 验证。

## 13. 阶段 6：P1 错误模型与子进程边界

> 实施状态：PR11 已实现并通过本地全部门禁（host 165 项测试）；Linux runtime CI 与 Android 真机验证仍属后续门禁。

### 13.1 结构化错误

当前大量路径使用 `Error::Msg(String)`。计划逐层替换，不一次性重写全部错误：

- [x] 建立带 `context`、`path` 和 `source: io::Error` 的 I/O 错误类型（`Error::IoContext(Box<IoError>)`）。
- [x] 为 Config、Scan、Plan、Mount、Storage、LKM、State 建立可匹配变体（Config/Scan/Plan 复用 PR4/PR5 已落地的专项变体；新增 `Mount/Storage/Lkm/State(Box<ContextError>)` 与 `Subprocess`）。
- [x] Display 文本由错误类型生成，不在调用点提前拼成不可解析字符串（新增路径全部遵守；`Msg` 仅保留给尚未迁移的低风险路径）。
- [x] 在层边界统一翻译 rustix、procfs、serde 和子进程错误（`CausalError` 收敛）。
- [x] 明确永久错误、可重试错误和需要人工介入的错误（`ErrorClass` + 穷尽 `Error::classify()`）。
- [x] 主程序保持 `thiserror`，不引入 `anyhow`；`anyhow` 继续只用于 `xtask`/通知等应用边界。

### 13.2 子进程执行器

为 `e2fsck`、`mke2fs`、`getprop`、`insmod` 等生产路径建立统一 helper：

- [x] 记录程序、参数、工作目录、退出码或 signal（`ProcessError` 保留 program/args/cwd/`ExitStatus`）。
- [x] stdout/stderr 使用有上限的 head+tail 缓冲，继续 drain，避免大输出 OOM 或管道死锁（`OutputCapture`，默认每流 32 KiB）。
- [x] 对无需捕获输出的命令只检查 status，不额外分配缓冲（`CaptureMode::None` 使用 `Stdio::null`）。
- [x] 不通过通用 shell 拼接命令，不接受未验证的命令字符串（`CommandSpec` 逐参数传入）。
- [x] 每个调用点显式声明可接受退出码（`ExitPolicy::{Success,Accepted,Any}`；`Any` 仅用于 insmod -EAGAIN 语义）。
- [x] 敏感环境变量和 Telegram token 不进入 Debug、错误或日志（`CommandSpec` 手工 Debug 对 env 值做 `<redacted>`；`ProcessError` 不保存 env）。

### 13.3 阶段验收

- [x] 高风险错误可以通过 enum variant 精确断言（`Error::Mount/Storage/Lkm/State/IoContext/Subprocess`）。
- [x] 子进程失败日志同时保留开头上下文和结尾错误信息（head + omitted 标记 + tail）。
- [x] 不存在无限 stdout/stderr 内存增长（固定 8 KiB 读块 + 32 KiB 容量 + 继续 drain）。

## 14. 阶段 7：P1 状态真实性与协议回归

### 14.1 最终状态

- [ ] `mounted_modules`、错误计数和 active mount 列表从最终执行结果生成。
- [ ] 执行器统计与最终 mountinfo 分开保存，避免把“尝试成功”当成“仍然活跃”。
- [ ] rollback 后重新计算状态，不沿用 rollback 前快照。
- [ ] 状态中记录失败阶段、回滚状态和未清理资源列表。
- [ ] `.old` 或历史启动数据与当前启动 session 明确区分。

### 14.2 稳定协议

- [ ] 为所有 CLI JSON 建立 golden/snapshot 测试。
- [ ] 新增字段保持向后兼容；删除或重命名字段需要迁移期。
- [ ] `show-config` 与 `save-config` 对 null、缺失值、未知字段保持一致。
- [ ] WebUI 不拥有第二套模块过滤或状态计算逻辑。

### 14.3 阶段验收

- [ ] `status` 可以解释“挂载失败”“挂载成功但回滚”“回滚不完整”三种情况。
- [ ] 旧 WebUI 对新增字段仍可正常运行。
- [ ] 状态文件写入保持原子性。

## 15. 阶段 8：P2 代码复用与依赖收敛

### 15.1 优先复用现有依赖

- [ ] 用 `rustix::fs::lgetxattr/lsetxattr` 替代 `extattr`。
- [ ] 用 `rustix::fs::chownat(..., SYMLINK_NOFOLLOW)` 替代手写 `libc::lchown`。
- [ ] 用 `rustix::io::Errno::BUSY/LOOP` 替代直接 `libc` 常量。
- [ ] 删除直接 `extattr` 和 `libc` 依赖，并确认目标构建通过。
- [ ] 对 rustix xattr 读取采用正确的长度查询/缓冲策略，测试长 SELinux context。

### 15.2 构建依赖瘦身

- [ ] 将 `zip` 改为 `default-features = false`，只启用 `deflate-flate2-zlib-rs`。
- [ ] 比较修改前后 `cargo tree -p xtask`、首次构建时间和 release ZIP 内容。
- [ ] 将通知工具 Tokio 从 `full` 收敛到实际需要的 feature，至少保留 `rt-multi-thread`。
- [ ] 验证通知发送、multipart 上传和超时路径。

### 15.3 条件性 crate 候选

| crate | 使用条件 | 当前决定 |
| --- | --- | --- |
| `xshell` | `xtask` 命令继续增加，需要统一 cwd/env/status | 条件引入，仅限 `xtask` |
| `walkdir` | 同时替代 staging copy 与 ZIP 递归，并删除 `fs_extra` | 条件引入 |
| `serde_with` | 三态 patch 字段增加到多个 | 暂不引入 |
| `sha2` | LKM 运行时哈希验证 | 建议引入 |
| `tempfile` | 能保留 Android 回退目录、显式清理错误和 retain 语义 | 当前不替换运行时实现 |

### 15.4 明确不引入

- [ ] 主程序不引入 `anyhow`。
- [ ] 主程序 CLI 不引入 `clap`。
- [ ] 不引入 `regex` 只为验证 Module ID。
- [ ] 不同时混用 `nix` 与 `rustix`。
- [ ] 不引入 `env_logger` 取代当前很小的 Android/host logger。
- [ ] 不用 `scopeguard` 取代需要返回错误和聚合诊断的挂载事务。

### 15.5 阶段验收

- [ ] 主程序直接依赖数量下降，且没有功能回归。
- [ ] 唯一手写 `lchown` FFI `unsafe` 被移除。
- [ ] `xtask` 不再编译未使用的 ZIP 加密和压缩算法。

## 16. 阶段 9：P1/P2 测试架构与故障注入

### 16.1 测试组织

- [ ] 超过约 400 行或测试体明显大于实现的模块，把 inline tests 移到相邻 `*_tests.rs`。
- [ ] 保持 `#[path = "foo_tests.rs"] mod tests;`，不为测试扩大生产 API 可见性。
- [ ] 建立共享但单一职责的 fake：mount ops、process runner、filesystem fault injector。
- [ ] 不创建包含所有 helper 的万能 `test-utils` crate；只有跨多个 workspace member 复用时再拆。

### 16.2 必需单元测试

- [ ] Config missing/corrupt/I/O error/atomic save。
- [ ] ModuleId、重复模块和目录名不一致。
- [ ] Plan 冲突、路径优先级和确定性排序。
- [ ] Overlay 64 层边界与 child mount rollback。
- [ ] Magic 各操作类型和所有失败分支。
- [ ] MountTransaction commit/disarm/rollback/rollback-failure。
- [ ] LKM 候选、哈希和 boot-guard。
- [ ] CLI JSON snapshots。
- [ ] ZIP 排序、权限、输出文件排除和可重复构建。

### 16.3 Linux namespace 集成测试

- [ ] 在 disposable mount namespace 中验证 tmpfs、bind、move 和 lazy unmount。
- [ ] 测试嵌套子挂载的深度优先卸载。
- [ ] 测试 mountinfo 解析与最终确认。
- [ ] 无 CAP_SYS_ADMIN 时明确 skip，而不是伪装 pass。

### 16.4 Android 真机矩阵

| 维度 | 至少覆盖 |
| --- | --- |
| Root 框架 | KernelSU、APatch |
| 后端 | Overlay only、Magic only、混合模式 |
| storage | tmpfs、ext4 loop |
| tmpfs xattr | 支持、不支持 |
| 架构 | arm64 真机；armv7/x86_64 至少交叉编译 |
| 内核线 | 每个声明支持 LKM 的内核/GKI 组合 |
| Android | 项目声明支持的最低版本和当前主版本 |
| 故障 | 损坏配置、冲突模块、空间不足、卸载繁忙、LKM 熔断 |

### 16.5 阶段验收

- [ ] 单元测试可以稳定复现每个已知失败分支。
- [ ] 真机报告包含设备、Android、内核、root 框架、SELinux 状态和最终 mountinfo。
- [ ] 不把交叉编译成功写成真机验证成功。

## 17. 阶段 10：P2 可观测性与性能基线

### 17.1 低开销阶段计时

- [ ] 复用项目既有的粗粒度计时设计；若当前分支不存在，则以独立小 PR 恢复。
- [ ] 记录 startup、config、scan、plan、storage、Overlay、Magic、state、rollback、cleanup。
- [ ] 正常完成记录 `status=ok`，异常离开作用域记录 `status=aborted`。
- [ ] 默认日志级别为 `info`；逐模块和逐挂载细节放在 `debug`。
- [ ] 日志不包含 Telegram token、完整敏感环境或不必要的用户路径内容。

### 17.2 性能检查

- [ ] 记录模块数量、节点数量、计划操作数量和各阶段耗时。
- [ ] 在相同设备、相同模块集上至少运行 10 次，报告中位数和 P95。
- [ ] 比较挂载事务与额外 mountinfo 确认带来的开销。
- [ ] 若启动耗时回归超过约 5%，必须解释并评估是否可降低粒度或系统调用次数。

### 17.3 阶段验收

- [ ] 一条失败日志可以定位阶段、目标、后端、错误和回滚结果。
- [ ] 默认日志量不会随文件节点数量线性爆炸。

## 18. 阶段 11：CI、发布与文档

### 18.1 CI 门禁

每个 PR 至少执行：

```text
cargo fmt --all -- --check
cargo metadata --locked --no-deps
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo check --locked --target x86_64-unknown-linux-gnu
cargo check --locked --target aarch64-linux-android
cargo check --locked --target armv7-linux-androideabi
cargo check --locked --target x86_64-linux-android
git diff --check
```

另外：

- [ ] dependency-only 变更比较 `Cargo.lock` 和 feature tree。
- [ ] Linux namespace 集成测试使用独立 job，并明确 capability 要求。
- [ ] release job 校验 ZIP 文件列表、权限、哈希和可重复性。
- [ ] 真机测试作为受控发布门禁或发布候选验收，不伪装成普通 CI。

### 18.2 发布与回退

- [ ] 每个高风险 PR 都记录回退提交或 feature-independent 回退方案。
- [ ] 配置语义变化提供迁移说明和损坏配置恢复步骤。
- [ ] LKM 支持矩阵、哈希和设备验证记录同步更新。
- [ ] changelog 区分内部重构、行为修复和兼容性变化。
- [ ] 发布前对 rescue boot、禁用模块和手动清理残留挂载步骤做演练。

## 19. 推荐 PR 切分顺序

| PR | 内容 | 风险 | 前置条件 | 状态 |
| --- | --- | --- | --- | --- |
| 1 | 固定工具链策略、记录基线 | 低 | 无 | 待执行（用户未选择） |
| 2 | `rustix` 收敛，删除直接 `libc/extattr` | 低-中 | Android target check | 待执行 |
| 3 | `zip`/Tokio feature 瘦身 | 低 | 构建产物比较 | 待执行 |
| 4 | Config 原子写与损坏配置 fail-closed | 中 | CLI/WebUI 错误契约 | ✅ 已提交（`b11bb03e`，含 G03/G04/G07） |
| 5 | `ModuleId` 与重复模块拒绝 | 中 | 扫描回归测试 | ✅ 已提交（含 G08） |
| 6 | `is_mounted -> Result<bool>` 与 mount ops 边界 | 中 | fake mount ops | ✅ 已提交（与 PR7 合并，G15） |
| 7 | `MountTransaction` 基础设施，不改变执行顺序 | 中-高 | 故障注入框架 | ✅ 已提交（与 PR6 合并，G15） |
| 8 | Overlay/Magic 跨阶段回滚 | 高 | Linux namespace 测试 | ✅ 已提交（含 G13） |
| 9 | Magic executor 错误语义和测试 | 高 | 真机候选测试 | ✅ 已实现（真机候选验证留待发布门禁） |
| 10 | LKM 哈希、override 限制和熔断诊断 | 高 | 支持矩阵、设备回退 | ⛔ 放弃（用户决定，2026-08-29） |
| 11 | 结构化错误与子进程 runner | 中 | 前述行为稳定 | ✅ 已实现（本地门禁通过，待提交/push/CI） |
| 12 | 状态真实性、CLI snapshots、文档 | 中 | 执行器结果模型稳定 | 待执行 |
| 13 | 计时与性能基线 | 低-中 | 核心修复完成 | 待执行 |

高风险 PR 不应与依赖升级、格式化全仓库或无关重命名混合。

## 20. 风险登记表

| 风险 | 可能影响 | 缓解措施 |
| --- | --- | --- |
| fail-closed 使旧损坏配置不再自动启动 | 用户认为功能突然失效 | WebUI 明确错误、恢复命令、迁移文档 |
| 统一回滚改变现有保留挂载行为 | 启动或软重启行为变化 | 明确 `disable_umount` 契约、真机 A/B |
| Module ID 严格化拒绝历史异常模块 | 部分第三方模块被跳过 | 提前扫描告警、清晰诊断、不要自动改目录 |
| rustix xattr 缓冲使用错误 | SELinux context 读取失败 | 两阶段长度读取、长值测试、Android 验证 |
| ZIP feature 精简改变压缩结果 | 发布包大小或兼容性变化 | 解压测试、文件列表/权限/hash 对比 |
| Tokio feature 精简遗漏驱动 | 通知工具运行失败 | notify 集成测试和实际测试发送 |
| LKM 哈希或矩阵错误 | 拒绝可用模块或加载错误模块 | 构建时生成、发布时复核、未知组合拒绝 |
| 测试 fake 与真实 syscall 偏离 | 单元测试假通过 | Linux namespace + Android 真机双层验证 |

## 21. 完成定义

只有同时满足以下条件，全面审计整改才算完成：

- [ ] 配置损坏不再静默使用默认 Overlay。
- [ ] 配置和状态文件使用原子写入。
- [ ] 模块 ID 在类型边界验证，重复和目录不匹配被拒绝。
- [ ] 任意后端或后续阶段失败都会触发全流水线回滚。
- [ ] 回滚后通过 mountinfo 重新确认，不把查询错误当作未挂载。
- [x] Magic executor 不再吞掉直接子节点或只读重挂载失败。
- [ ] Overlay 子挂载按深度正确卸载。
- [ ] LKM 候选选择、哈希验证和 boot-guard 全部生效。
- [ ] 直接 `libc/extattr` 依赖被现有 `rustix` 能力替代。
- [ ] `zip` 和 Tokio 只启用实际需要的特性。
- [x] 关键错误为可匹配的结构化类型，而不是只剩字符串（PR11：高风险路径已迁移，`Msg` 仅留低风险路径）。
- [ ] 关键失败路径具备故障注入测试。
- [ ] CI、Linux namespace 与 Android 真机验证边界被明确记录。
- [ ] 稳定 CLI、JSON 字段和路径没有未经迁移的破坏性变化。
- [ ] 最终文档包含设备验证结果、已知限制和回退步骤。

## 22. 技能复核补遗（2026-08-29）

> 本节由 `openai-codex-rust-patterns` 技能复核产生，补充原计划未覆盖或表述不足的缺口。
> 编号 G01–G15 在开工前逐条并入对应阶段验收表；对应阶段已通过 [x] 标记的不再重复列为缺口。

### 22.1 P0/P1 语义缺口

- [ ] **G01（并入 14.1）**：`scan.ret` 的 `AppModule.is_mounted` 必须由最终执行结果/mountinfo 生成，禁止用计划选择冒充挂载成功。当前 `src/state.rs` 的 `app_modules()` 按 plan 计算该字段，执行失败后 WebUI 仍可能显示 `true`。
- [ ] **G02（并入 14.1 / 7.1）**：`RunState::load_or_default` 区分状态文件缺失、损坏与 I/O 错误；损坏状态不得静默回退默认，至少产生可查询诊断。当前解析失败只 `log::warn` 后返回默认。
- [x] **G03（并入 8.1）**：`default_mode = "ignore"` 等"可解析但已废弃的值"不得静默规范化为 Overlay；已改为解析/patch/save 三处显式报错（PR4）。
- [x] **G04（并入 8）**：`module_blacklist.toml` 语义已定为"缺失=无黑名单（正常），损坏/不可读=错误并 fail-closed"，测试已覆盖（PR4）。
- [ ] **G05（并入 10.3 / 10.4 / 11.3）**：清理 helper 的 mountinfo 探测失败必须返回错误；清理后必须重新读 mountinfo 确认目标消失；禁止"未探测到 = 已清理"。当前 `is_mounted` 错误返回 `false`，会跳过 unmount 却返回 `Ok`。
- [ ] **G06（并入 11.3）**：tmpfs → ext4 回退前必须验证 tmpfs 已卸载；卸载失败则 fail-closed，不再尝试 ext4。当前 `storage/mod.rs` 对卸载失败只告警后继续。
- [x] **G07（并入 8.2）**：父目录 `sync_all` 失败按保存失败处理；非 Unix 回退已注明仅用于 host 测试且无崩溃安全保证（PR4）。
- [x] **G08（并入 9.2）**：scanner 根 `read_dir` 失败（当前静默返回空列表）与 walk 内 `read_dir`/`symlink_metadata` 失败（当前无日志）必须按"跳过/警告/致命"分类并记录。已实现：模块根不可读为致命 `ScanReadDir`，条目级错误警告跳过（PR5）。
- [ ] **G09（并入 11.3 / 7.1）**：`reset_image_files` 停止按文件名前缀删除，改为精确文件名或保留后缀白名单，避免删除用户 `modules.img.*` 备份。
- [ ] **G10（并入 12）**：定义 LKM nuke 失败语义——hash/矩阵/加载失败是中断流水线，还是"记录到 `state.json` 的显式降级"；两者必选其一并写验收标准。当前 `nuke_ext4_sysfs` 返回 `()`、失败仅告警。

### 22.2 技能强化项

- [x] **G11（并入 13.2）**：统一子进程 runner 增加"子进程总超时 + I/O drain 独立超时"；把 `ksud`/`apd`（`module_status.rs`）也纳入 runner 范围。
- [x] **G12（并入 13.1）**：用"瞬态/永久/需人工介入"三分类 + 穷尽 `classify()` match 编码重试分类，避免只靠文档约定。
- [x] **G13（并入 10.5）**：故障注入用 `AtomicBool` 门控（与现有 KSU/xattr 缓存风格一致），不引入 cargo feature。
- [ ] **G14（并入 14.2）**：serde 改名/删字段采用 `rename + alias` 与可解析 tombstone（现有 `legacy_custom_mounts` 已是范例）；内部错误 enum 与 CLI/WebUI 线格式错误分离。
- [x] **G15（并入 19 PR 6/7）**：`is_mounted -> Result<bool>` 与 `MountTransaction` 基础设施合并落地——可传播路径用查询 API，`Drop` 路径用 best-effort helper，避免 PR 6 单独触碰 5 个调用点中的 3 个 Drop 路径。

### 22.3 复核确认（已完成）

- [x] 基线 `ba06309d` 复验：fmt、metadata、clippy、`cargo test`（110 = 106+3+1）全部通过。
- [x] stable 1.97 `cargo check` 与 workspace 测试通过。
- [x] `x86_64-unknown-linux-gnu` 与 `aarch64-linux-android` `cargo check` 通过。
- [x] §4 全部基线论断、§5–§12 的关键诊断均经代码逐条核实。
- [x] PR4 完成后复验：fmt/clippy 通过，workspace 测试通过（host 113+3+1=117；Linux 另含 cfg(unix) 符号链接测试），stable 1.97 与 x86_64-linux-gnu/aarch64-linux-android check 通过。
- [x] PR5 完成后复验：fmt/clippy 通过，workspace 测试通过（host 124+3+1=128；Linux 另含 cfg(unix) 符号链接测试），x86_64-linux-gnu/aarch64-linux-android check 通过。
- [x] PR6/7 完成后复验：fmt/clippy 通过，workspace 测试通过（host 138+3+1=142；Linux 另含 cfg(unix) 与 mountinfo 测试），x86_64-linux-gnu/aarch64-linux-android check 通过。
- [x] PR8 完成后复验：fmt/clippy（host 与 Linux target all-targets）通过，workspace 测试通过（host 147+3+1=151），x86_64-linux-gnu/aarch64-linux-android check 通过。
- [x] PR9 本地复验：fmt、host/Linux-target clippy、workspace host 测试（147+3+1=151）、Magic Linux-only 测试编译，以及 x86_64 Linux 与 aarch64/armv7/x86_64 Android check 通过；Linux runtime CI 与 Android 真机验证边界未混淆。
- [x] PR11 本地复验：fmt、host/Linux-target clippy、workspace host 测试（165+3+1=169），以及 x86_64-linux-gnu/aarch64-linux-android/armv7-linux-androideabi/x86_64-linux-android check 通过；runner 单测覆盖 head+tail 上限、总超时、drain 超时、退出码策略与 env Debug 脱敏。

## 23. 最终交付物

- 分阶段合并的 Rust 修复与重构 PR。
- 单元测试、故障注入测试和 Linux namespace 集成测试。
- Android 真机验证矩阵与原始关键日志摘要。
- 更新后的 `ARCHITECTURE.md`、配置迁移说明和 LKM 支持矩阵。
- 依赖树、构建时间、ZIP 大小和启动延迟的前后对比。
- 发布候选验收报告，以及可执行的回退与 rescue boot 步骤。
