# Rust 后端审查：未闭环项

原审查报告（2026-08-31，提交 `070af486`）的完整正文已删除：其中的状态表、行号引用与
“剩余 P1/P2/P3”计数在 71 个提交后与代码不符，反而会误导。本文件只保留仍然成立、
且没有其它地方记录的未闭环项。

已闭环节目的权威记录是 [`changelog.md`](../changelog.md)；当前代码状态以源码与测试为准。

## 仍未闭环

- **HM-RUST-005 — 符号链接的 stat / xattr follow policy 不一致。**
  `src/sys/fs.rs` 的 `clone_entry_metadata` 对目录符号链接取跟随后的 stat，但用
  `lgetfilecon` / `lsetfilecon` 读取与写入 SELinux 上下文；同一份元数据里的 uid/gid 走
  `chownat(..., SYMLINK_NOFOLLOW)`，SELinux 上下文却指向链接之外的对象。需要在
  `src/sys/fs.rs`、`src/magic_mount/exec.rs` 与 `src/utils/mod.rs` 三处确认“跟随目录链接”
  与“不跟随文件链接”的边界，统一成一个明确策略后再补齐回归测试。

- **HM-RUST-012 — scanner 记录与 staging 使用之间存在类型竞态。**
  `src/scanner.rs` 记录的节点类型在 `src/sys/fs.rs` 物化时没有重新核对；模块源目录在扫描与
  被读取之间若被改写，staging 可能按旧类型处理新对象。模块源目录按约定是只读输入，因此这是
  纵深防御而非已知可利用路径，但仍是未闭环项。

## 设备验证边界

以上两项与所有挂载、loop、SELinux、KernelSU/APatch 交互一样，主机测试与三架构交叉编译都不能
替代真机验证。相关审查清单见 `.dsh/skills/hm-mount-safety-review/`。
