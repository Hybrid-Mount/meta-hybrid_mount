---
name: rust-guidelines
description: >
  Microsoft Rust Guidelines 的选题集（原 microsoft/rust-guidelines）：把「什么是
  panic、什么是错误」当成类型与契约问题来裁，并为 unsafe、FFI、日志、lint 覆盖、
  魔法值给出可引用的条目式判据。用于 Hybrid Mount 的错误处理分层与 unsafe/FFI 审查：
  src/errors.rs、src/sys/**、src/vfs/**、以及 workspace 级 lint 覆盖。
whenToUse: >
  当你要决定「这里该返回 Result 还是 panic」、要给 `#[allow]` 换成 `#[expect]`、
  要判断某段 unsafe 是否可能 unsound、或要给挂载/ioctl 常量补文档时读取。若只是
  常规命名、所有权、性能优化，优先读已装的 rust-skills（265 条规则，覆盖更广）。
license: MIT
metadata:
  vendored:
    source: https://github.com/microsoft/rust-guidelines
    commit: 19723b3
    upstream-path: src/guidelines
    vendored: "2026-09-30"
    curated: 见下方「安装范围与取舍」
---

# Microsoft Rust Guidelines（选题集）

上游是一本 mdbook（12 章、107 条规则、157 KB）。本仓库已装 `rust-skills`（265 条规则、26 个类别，含 unsafe / api / perf / lint / err / doc），因此这里**只取与已有规则互补、且与本项目代码形态相关的 4 章**，共 21 条规则、约 50 KB。

## 安装范围与取舍（为什么只有这些）

| 章节 | 收录 | 理由 |
| --- | --- | --- |
| `correctness/` | ✅ 7 条 | panic 哲学（何为 bug、何为错误）是 `rust-skills` 的 `err` 类目没有正面回答的问题；本项目 `unwrap_used`/`expect_used` 全 workspace deny，必须有一套「什么时候本来就该 panic」的判据 |
| `universal/` | ✅ 11 条 | `M-LINT-OVERRIDE-EXPECT`、`M-DOCUMENTED-MAGIC`、`M-LOG-STRUCTURED` 直接对应当前代码里的 `#[allow]`、挂载/ioctl 常量与 `log` 调用 |
| `ffi/` | ✅ 3 条 | `rust-skills` 没有 FFI 类目；本项目的 `libc`/ioctl 边界需要它 |
| `checklist/` | ✅ 1 条 | 上游自带的跨章速查表，当索引用 |
| `libs/`（38 条）、`docs/`、`performance/`、`macros/`、`ai/`、`apps/`、`project/` | ❌ | 与 `rust-skills` 的 `api`/`doc`/`perf`/`macro`/`proj`/`test` 类目高度重复，且本项目是二进制 crate 而非发布给第三方的库 |

上游那份 136 KB 的 `src/agents/all.txt` 单体文件没有收录：它的信息量等于全书，而全书本身已经被裁到 50 KB。

## Hybrid Mount precedence（先读这里）

这些是通用的 API 设计建议，本仓库自己的契约优先。冲突时以 `CLAUDE.md` 与现有代码为准。

- **「检测到编程错误就 panic」不能推广成「可以 `unwrap()`」。** `Cargo.toml` 的 workspace lint 对 `unwrap_used`/`expect_used` 是 `deny`，`clippy.toml` 只在 `#[cfg(test)]` 下放开（`allow-unwrap-in-tests = true`）。`M-PANIC-ON-BUG` 说的是"不要给不可恢复的 bug 造 `Error` 类型"，不是"允许在生产路径 unwrap"。生产代码用 `Result` + `?`，真的不可恢复就用 `panic!` 并写清消息（`M-PANIC-MESSAGE`）。
- **分层已经给定，不要照搬书里的 crate 划分。** `src/errors.rs` 定义 `Error`，`src/sys/**` 是系统调用与 ABI 层，`src/runtime/**` 是所有权账本与事务层。`M-FFI-TRANSLATES` 说的"逻辑放核心、边界只做翻译"在这里对应：规划与冲突检测留在 `src/plan/**`，`src/sys/**` 只负责把 syscall 结果翻译成 `Result`。
- **`M-STATIC-VERIFICATION` 不能替代本仓库实际门禁。** 真正的门禁是 `.github/workflows/lints.yml`（fmt、`clippy -D warnings`、`cargo test --workspace`、禁用符号、ShellCheck、Android 三架构）。改完代码以 `hm-verify` 的清单为准，而不是"看起来满足规则"。
- **主机编译通过不算数。** `src/main.rs` 有 `#![cfg_attr(not(any(target_os = "linux", target_os = "android")), allow(dead_code))]`，Android 专属分支在 Windows 上根本不编译。

## 规则索引

### correctness

- [Detected programming bugs are panics, not errors](guidelines/correctness/M-PANIC-ON-BUG.md) — 契约违背是 bug，panic；输入解析失败是错误，`Result`
- [Panic means 'stop the program'](guidelines/correctness/M-PANIC-IS-STOP.md)
- [Custom panics have a helpful message](guidelines/correctness/M-PANIC-MESSAGE.md)
- [Panic continuation is last resort](guidelines/correctness/M-PANIC-CONTINUATION.md) — `catch_unwind` 之前先想清楚
- [Unsafe needs reason, should be avoided](guidelines/correctness/M-UNSAFE.md)
- [Unsafe implies undefined behavior](guidelines/correctness/M-UNSAFE-IMPLIES-UB.md)
- [All code must be sound](guidelines/correctness/M-UNSOUND.md)

> 这三条 unsafe 规则与已装的 `rust-unsafe-review` 重叠，但更短、更适合当判据；要写 `# Safety` / safety 注释的证明义务时用那份，要判断"这段 unsafe 该不该存在"时用这份。

### universal

- [Lint overrides should use `#[expect]`](guidelines/universal/M-LINT-OVERRIDE-EXPECT.md) — 本项目现存 3 处 `#[allow(dead_code)]`
- [Magic values are documented](guidelines/universal/M-DOCUMENTED-MAGIC.md) — 挂载标志、ioctl 号、分区名
- [Use structured logging with message templates](guidelines/universal/M-LOG-STRUCTURED.md) — `log::info!` 的模板写法
- [Public types are Debug](guidelines/universal/M-PUBLIC-DEBUG.md)
- [Public types meant to be read are Display](guidelines/universal/M-PUBLIC-DISPLAY.md)
- [Prefer regular over associated functions](guidelines/universal/M-REGULAR-FN.md)
- [Names of items are short](guidelines/universal/M-SHORT-NAMES.md)
- [If in doubt, split the crate](guidelines/universal/M-SMALLER-CRATES.md)
- [Use static verification](guidelines/universal/M-STATIC-VERIFICATION.md)
- [Follow the upstream guidelines](guidelines/universal/M-UPSTREAM-GUIDELINES.md)
- [Names are free of weasel words](guidelines/universal/M-WEASEL-WORDS.md)

### ffi

- [FFI crates follow established naming conventions](guidelines/ffi/M-FFI-NAMING.md)
- [Business logic belongs in core crates, FFI only translates](guidelines/ffi/M-FFI-TRANSLATES.md)
- [Isolate DLL state between FFI libraries](guidelines/ffi/M-ISOLATE-DLL-STATE.md)

### 速查

- [跨章 checklist](guidelines/checklist/README.md) — 按"交付前自检"组织的条目，含未收录章节的标题，需要时可按标题回上游找

## 章节说明

- [guidelines/README.md](guidelines/README.md) — 上游全书目录与编写约定
- 每条规则文件保留上游原文、`<why>` / `<tip>` 标记与版权头（MIT），只做了目录裁剪，未改写内容。
- 未收录章节的链接在上游是锚点形式（如 `../libs/ux/#M-INIT-BUILDER`），裁掉后不会产生失效引用。
