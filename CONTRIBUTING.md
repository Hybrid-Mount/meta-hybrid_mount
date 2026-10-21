# Contributing / 参与贡献

Hybrid Mount welcomes code, documentation, translations and device compatibility reports.
欢迎提交代码、文档、翻译与设备兼容性反馈。

Read the [Usage Notice](USAGE_NOTICE.md) and [中文文档](docs/README_ZH.md) first.
Search existing issues before reporting a problem. Include module version, device and kernel
version, KernelSU/APatch version, reproduction steps, expected behavior and a bugreport.
Remove credentials and personal information from logs before uploading them.

Use one of the repository issue templates, including its title prefix and required sections.
All issues are checked, including submissions through the CLI/API without labels. Bug reports
and blacklist requests must include a log/archive link or pasted diagnostic output in the
Upload Area; a long explanation or an unrelated website link does not count as a log.
Incomplete reports receive a `needs-template` label and are closed with a list of missing
information. Edit the existing issue to complete it; the bot reopens issues it closed once
the check passes. Passing this check does not confirm the report's technical conclusions.

请使用仓库提供的 Issue 模板，保留标题前缀并填写必填章节。所有 Issue 均会检查，包括通过
CLI/API 提交且没有标签的 Issue。错误报告和黑名单申请须在上传区域提供日志或报告附件链接，
或粘贴日志输出；长篇推测或无关网页链接不能代替日志。不完整的报告会标记 `needs-template`，
列出缺项并自动关闭。请直接编辑原 Issue 补全，由机器人关闭的 Issue 在检查通过后会自动重新
打开。检查通过仅代表格式完整，不代表技术结论已经确认。

- [AtomGit Issues](https://atomgit.com/YuzakiKokuban/meta-hybrid_mount/issues)
- [GitHub Issues](https://github.com/Hybrid-Mount/meta-hybrid_mount/issues)

For code changes, fork the repository, create a branch from `dev` and keep each change focused.
Submit a PR on either platform and describe the problem, changed behavior and verification.
AtomGit PRs require maintainer integration into GitHub before the next automatic sync;
please do not open duplicate PRs on both platforms.

Rust changes should pass `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`.
For WebUI changes, run `pnpm install --frozen-lockfile`, `pnpm lint`, `pnpm test`
and `pnpm build` from `webui/`. Changes involving kernel modules or boot-time mounts
should describe the real-device verification and the tested kernel/framework versions.
Documentation-only changes should check links and examples.

Preserve existing license and attribution notices. See [LICENSE](LICENSE),
[webui/LICENSE](webui/LICENSE) and [THIRD_PARTY.md](THIRD_PARTY.md) for component licenses.
