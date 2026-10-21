# Contributing / 参与贡献

Hybrid Mount welcomes code, documentation, translations and device compatibility reports.
欢迎提交代码、文档、翻译与设备兼容性反馈。

Read the [Usage Notice](USAGE_NOTICE.md) and [中文文档](docs/README_ZH.md) first.
Search existing issues before reporting a problem. Include module version, device and kernel
version, KernelSU/APatch version, reproduction steps, expected behavior and a bugreport.
Remove credentials and personal information from logs before uploading them.

Use an issue template with its title prefix and required fields, including for CLI/API submissions.
Bug reports and blacklist requests need logs in the Upload Area. Incomplete issues are labeled
`needs-template` and closed; edit the original issue to let the bot reopen it after validation.

通过 CLI/API 提交也须遵循 Issue 模板的标题前缀和必填字段。错误报告和黑名单申请须在上传
区域提供日志。缺项会标记 `needs-template` 并关闭；编辑原 Issue 补全后，机器人会重新打开。

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
