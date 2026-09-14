---
name: hm-mount-safety-review
description: Hybrid Mount 挂载与文件操作代码审查清单：返回值检查、Path 使用、回滚资源清理、只读源目录、禁用符号、cfg 平台隔离与 ext4/LKM 熔断语义。
whenToUse: 评审或修改 mount、unmount、文件系统、staging、回滚相关代码，或用户要求 review 这类改动时。
---

# 挂载安全审查清单

只读原则与事务性是本项目的底线。逐条核对，发现问题先给最小修复与验证方式。

## 系统调用与文件操作

- [ ] 每次 mount / umount2 / open / mkdir / symlink / rename 都检查返回值，错误向上传播（`Result` + `?`，`src/errors.rs`）。
- [ ] 路径使用 `Path`/`PathBuf`，没有字符串拼接路径。
- [ ] 不把输入直接拼进 shell 命令。
- [ ] 平台相关代码用 `#[cfg(any(target_os = "linux", target_os = "android"))]` 隔离，非 Linux 给出清晰 `Error`。

## 只读源目录

- [ ] 绝不写入 `/data/adb/modules/<id>` 源目录；一切 staging 写入运行目录。

## 回滚与资源

- [ ] 失败路径清理**所有**已创建的挂载点与临时目录，顺序 LIFO。
- [ ] 不遗留 loop 设备、fd、tmpfs 挂载。
- [ ] 回滚后仍保留失败快照（`state.json`）供 WebUI 查询。
- [ ] ext4 模式：`NukeExt4Sysfs` / LKM 路径失败处理完整；LKM 熔断标记在 `insmod` 前持久化、正常返回后移除。

## 代码规范门禁

- [ ] 无 unwrap() / expect() / dbg! / todo! / unimplemented!（workspace lint deny）。
- [ ] 无 kasumi、无 normalize_symlinked_partition_layout / normalize_module_layout。
- [ ] 新增配置字段已同步 `module/config.toml` 与文档。
- [ ] 新增跨模块冲突检测放在 planner，executor 不裁决。

## 快速自查

```bash
# 权威门禁：非测试代码中的禁用符号会被 clippy 拒绝
cargo clippy --workspace --all-targets -- -D warnings

# 禁用符号
grep -Rni -E 'kasumi|normalize_symlinked_partition_layout' src module xtask
```

`#[cfg(test)]` 模块与 `*_tests.rs` 里的 unwrap/expect 是测试夹具，属预期，不要按 grep 结果改测试；以 clippy 为准。

高危项（泄漏挂载、静默失败、路径拼接）必须阻断合入。
