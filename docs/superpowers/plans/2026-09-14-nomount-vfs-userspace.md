# NoMount VFS 后端（用户态子系统）实施计划

> **面向 Agent 执行者：** 必需子技能：使用 superpower-subagent-driven-development（推荐）或 superpower-executing-plans 按任务逐项执行本计划。步骤使用复选框（`- [ ]`）语法进行跟踪。

**目标：** 在 Hybrid Mount 中实现 `vfs` 第三后端的用户态部分：把共享挂载树映射为 NoMount wire 规则，驱动唯一活动的内核 Provider（K1 或 K2），并接入规划、状态、CLI 与 WebUI。

**架构：** 新增 `src/vfs/` 模块：`rule.rs`（树→规则纯函数）、`protocol.rs`（`nm_payload` 字节级编解码）、`sys.rs`（keyring `add_key` 发送）、`backend.rs`（Provider 选择与二选一互斥）、`exec.rs`（应用规则与统计）。配置层新增 `Mode::Vfs`，规划器标注并校验遮蔽冲突，流水线在 magic 之后执行 vfs 阶段，状态与 WebUI 暴露 Provider 与注入计数。

**技术栈：** Rust（edition 2024，nightly），`libc`（新增，仅 linux/android，用于 `add_key` 与 `mmap`），现有 workspace lints。

**规格：** `docs/superpowers/specs/2026-09-14-nomount-vfs-backend-design.md`（执行者必须同时阅读规格；本计划只覆盖规格 §8–§12、§14–§15 的用户态部分）。

**范围说明：** 规格 §13「独立内核实现子系统（K2）」以及 §7.1 的内核集成脚本属于独立子系统，另立计划 `docs/superpowers/plans/2026-09-14-nomount-vfs-kernel.md`，不在本计划内。

## 全局约束

- 许可证 `GPL-3.0-only`；每个新建 `.rs` 文件首行必须是 `// SPDX-License-Identifier: GPL-3.0-only`。
- 禁用符号（workspace lint `deny`）：`dbg!`、`expect()`、`todo!`、`unimplemented!`、`unwrap()`。测试代码例外（`clippy.toml` 允许 tests 内 unwrap/expect）。
- 生产代码用 `Result` + `?`；可结构化错误必须新增 `Error` 变体，不得用 `Error::msg` 预格式化。
- 路径操作一律 `Path`/`PathBuf`，不得字符串拼接。
- `/data/adb/modules/<id>/**` 始终是只读输入，不得写入。
- 面向 WebUI 的 JSON 只走 stdout，诊断日志走 `log` crate。
- 新配置字段必须同步 `module/config.toml` 与 `docs/ARCHITECTURE.md`。
- wire 常量必须与规格完全一致：`MAGIC=0x4E4F4D4F554E54`、`PAYLOAD_LEN=4096`、`BUFFER_LEN=4068`、`RULE_HEADER_LEN=12`、`DEL_HEADER_LEN=6`、flags `IS_DIR=1 / VIRTUAL_DIR=2 / WHITEOUT=4`、命令号 `GET_VERSION=1 … GET_UIDS=10`。
- Provider 二选一为硬不变量：同一时刻只驱动 K1 或 K2 之一，不并存、不热切换。
- 支持的协议版本集合初始为 `{"20"}`。

---

## 文件结构

| 文件 | 职责 | 动作 |
| --- | --- | --- |
| `src/vfs/mod.rs` | 子模块导出 | 新建 |
| `src/vfs/rule.rs` | `MountTree → Vec<VfsRule>` 纯函数 | 新建 |
| `src/vfs/rule_tests.rs` | 规则映射单测 | 新建 |
| `src/vfs/protocol.rs` | `nm_payload` 字节级编解码与批量 | 新建 |
| `src/vfs/protocol_tests.rs` | 协议单测 | 新建 |
| `src/vfs/sys.rs` | page 对齐缓冲 + `add_key` 发送（linux/android） | 新建 |
| `src/vfs/backend.rs` | `VfsKernel` trait、`KeyringKernel`、Provider 选择与互斥 | 新建 |
| `src/vfs/backend_tests.rs` | Provider 选择单测（mock） | 新建 |
| `src/vfs/exec.rs` | 应用规则、UID、统计 | 新建 |
| `src/vfs/exec_tests.rs` | 应用规则单测（mock） | 新建 |
| `src/config.rs`、`src/config_tests.rs` | `Mode::Vfs`、`vfs_strict`、`vfs_isolate_uids` | 修改 |
| `src/plan/mod.rs` | `vfs_module_ids`、遮蔽冲突 | 修改 |
| `src/state.rs` | `vfs_modules`、`vfs_active_mounts`、`vfs_provider`、`ModeStats.vfs` | 修改 |
| `src/errors.rs` | VFS 结构化错误变体 | 修改 |
| `src/pipeline.rs` | vfs 阶段、Provider 会话、回滚、状态装配 | 修改 |
| `src/main.rs` | `mod vfs;` | 修改 |
| `Cargo.toml` | `libc` 依赖（仅 linux/android） | 修改 |
| `module/config.toml`、`docs/ARCHITECTURE.md` | 文档同步 | 修改 |
| `webui/src/**` | 模式选项与状态展示 | 修改 |

---

## 任务 1：新增 `Mode::Vfs` 配置后端

**文件：**
- 修改：`src/config.rs:34-50`
- 测试：`src/config_tests.rs`

**接口：**
- 对外产出：`crate::config::Mode::Vfs`，`Mode::as_str(Mode::Vfs) == "vfs"`，serde 小写 `"vfs"`。

- [ ] **步骤 1：编写失败的测试**

在 `src/config_tests.rs` 末尾添加：

```rust
#[test]
fn vfs_is_accepted_as_global_and_path_mode() {
    let config = crate::config::Config::from_toml(
        "default_mode = \"vfs\"\n\n[rules.\"mod_a\".paths]\n\"system/etc/hosts\" = \"vfs\"\n",
    )
    .unwrap();
    assert_eq!(config.default_mode, crate::config::Mode::Vfs);
    assert_eq!(
        config.rules[&crate::module_id::ModuleId::try_from("mod_a").unwrap()].paths
            ["system/etc/hosts"],
        crate::config::Mode::Vfs
    );
}

#[test]
fn vfs_serializes_lowercase() {
    let toml = crate::config::Config::from_toml("default_mode = \"vfs\"\n")
        .unwrap()
        .to_toml()
        .unwrap();
    assert!(toml.contains("default_mode = \"vfs\""));
}
```

- [ ] **步骤 2：运行测试并确认其失败**

运行：`cargo test -p hybrid-mount vfs_is_accepted -- --nocapture`
预期：编译失败，提示 `no variant or associated item named 'Vfs'`。

- [ ] **步骤 3：编写最小实现**

修改 `src/config.rs` 的 `Mode`：

```rust
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Overlay,
    Magic,
    Vfs,
    Ignore,
}

impl Mode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Overlay => "overlay",
            Self::Magic => "magic",
            Self::Vfs => "vfs",
            Self::Ignore => "ignore",
        }
    }
}
```

同时更新 `src/config.rs` 顶部文档示例：

```toml
default_mode = "overlay"   # overlay | magic | vfs
```

- [ ] **步骤 4：运行测试并确认其通过**

运行：`cargo test -p hybrid-mount vfs_ -- --nocapture`
预期：两个测试 PASS。

- [ ] **步骤 5：提交**

```bash
git add src/config.rs src/config_tests.rs
git commit -m "feat(config): add vfs mount mode"
```

---

## 任务 2：规划器记录 vfs 模块

**文件：**
- 修改：`src/plan/mod.rs:30-40`（`MountPlan`）、`src/plan/mod.rs:127-139`（`PlanBuilder`）、`src/plan/mod.rs:210-239`（`finish`）、`src/plan/mod.rs:254-362`（`process_module`）
- 测试：`src/plan/mod.rs` 内 `mod tests`

**接口：**
- 依赖输入：任务 1 的 `Mode::Vfs`。
- 对外产出：`MountPlan.vfs_module_ids: Vec<ModuleId>`。
- 对外产出：`fn collect_vfs(module: &ModuleRecord, decisions: &[EntryDecision<'_>], builder: &mut PlanBuilder)`（私有，签名对齐现有 `collect_magic`）。

- [ ] **步骤 1：编写失败的测试**

在 `src/plan/mod.rs` 的 `mod tests` 内添加：

```rust
#[test]
fn vfs_module_is_recorded_in_plan() {
    let module = record("vfs_mod", &[("system/etc/hosts", false)]);
    let result = plan(&[module], &config(Mode::Vfs, no_rules()), &[]);
    assert_eq!(
        result.vfs_module_ids,
        vec![ModuleId::try_from("vfs_mod").unwrap()]
    );
    assert!(result.overlay_module_ids.is_empty());
    assert!(result.magic_module_ids.is_empty());
}
```

- [ ] **步骤 2：运行测试并确认其失败**

运行：`cargo test -p hybrid-mount vfs_module_is_recorded -- --nocapture`
预期：编译失败，提示 `no field 'vfs_module_ids'`。

- [ ] **步骤 3：编写最小实现**

在 `MountPlan` 增加字段（放在 `magic_module_ids` 之后）：

```rust
    pub magic_module_ids: Vec<ModuleId>,
    pub vfs_module_ids: Vec<ModuleId>,
```

在 `PlanBuilder` 增加字段：

```rust
    magic_module_ids: BTreeSet<ModuleId>,
    vfs_module_ids: BTreeSet<ModuleId>,
```

在 `finish(self)` 的 `MountPlan { .. }` 增加：

```rust
            magic_module_ids: self.magic_module_ids.into_iter().collect(),
            vfs_module_ids: self.vfs_module_ids.into_iter().collect(),
```

在 `collect_magic` 之后新增：

```rust
fn collect_vfs(
    module: &ModuleRecord,
    decisions: &[EntryDecision<'_>],
    builder: &mut PlanBuilder,
) {
    if !decisions
        .iter()
        .any(|decision| decision.mode == Mode::Vfs)
    {
        return;
    }

    builder.vfs_module_ids.insert(module.id.clone());
}
```

在 `process_module` 的两处 `collect_magic(...)` 调用旁各补一次 `collect_vfs(...)`（`src/plan/mod.rs:298` 与 `src/plan/mod.rs:360`）：

```rust
        collect_magic(module, &decisions, builder);
        collect_vfs(module, &decisions, builder);
        return Ok(());
```

```rust
    collect_magic(module, &decisions, builder);
    collect_vfs(module, &decisions, builder);
    Ok(())
```

- [ ] **步骤 4：运行测试并确认其通过**

运行：`cargo test -p hybrid-mount vfs_module_is_recorded -- --nocapture`
预期：PASS。

- [ ] **步骤 5：运行 planner 全量测试**

运行：`cargo test -p hybrid-mount plan:: -- --nocapture`
预期：全部 PASS（`MountPlan` 的 `Default` 派生自动包含新字段，不需要额外修改）。

- [ ] **步骤 6：提交**

```bash
git add src/plan/mod.rs
git commit -m "feat(plan): record vfs modules in mount plan"
```

---

## 任务 3：规划阶段拒绝 vfs 遮蔽冲突

**文件：**
- 修改：`src/plan/mod.rs:50-69`（`build_plan`）、新增 `ensure_vfs_not_shadowed`
- 测试：`src/plan/mod.rs` 内 `mod tests`

**接口：**
- 依赖输入：任务 2 的 `Mode::Vfs` 树标注。
- 对外产出：`fn ensure_vfs_not_shadowed(node: &MountNode, target: &str, ancestor_mount: Option<(Mode, &str)>) -> Result<()>`。

- [ ] **步骤 1：编写失败的测试**

在 `src/plan/mod.rs` 的 `mod tests` 内添加：

```rust
#[test]
fn vfs_file_under_overlay_directory_is_rejected() {
    let mut rules = no_rules();
    rules.insert(
        "alpha".to_owned(),
        crate::config::ModuleRule {
            default_mode: Some(Mode::Overlay),
            paths: BTreeMap::new(),
        },
    );
    rules.insert(
        "beta".to_owned(),
        crate::config::ModuleRule {
            default_mode: Some(Mode::Vfs),
            paths: BTreeMap::new(),
        },
    );
    let alpha = record("alpha", &[("system/etc", true)]);
    let beta = record("beta", &[("system/etc/hosts", false)]);
    let err = plan_err(&[alpha, beta], &config(Mode::Magic, rules));
    assert!(matches!(err, Error::PlanConflict { .. }), "unexpected: {err}");
}

#[test]
fn vfs_file_without_mounted_ancestor_is_allowed() {
    let mut rules = no_rules();
    rules.insert(
        "alpha".to_owned(),
        crate::config::ModuleRule {
            default_mode: Some(Mode::Overlay),
            paths: BTreeMap::new(),
        },
    );
    rules.insert(
        "beta".to_owned(),
        crate::config::ModuleRule {
            default_mode: Some(Mode::Vfs),
            paths: BTreeMap::new(),
        },
    );
    let alpha = record("alpha", &[("system/etc/other", true)]);
    let beta = record("beta", &[("system/etc/hosts", false)]);
    let result = plan(&[alpha, beta], &config(Mode::Magic, rules), &[]);
    assert_eq!(result.vfs_module_ids.len(), 1);
}
```

- [ ] **步骤 2：运行测试并确认其失败**

运行：`cargo test -p hybrid-mount vfs_file_under_overlay -- --nocapture`
预期：FAIL —— `build_plan` 返回 `Ok`，`assert!(matches!(..))` 断言失败。

- [ ] **步骤 3：编写最小实现**

在 `build_plan` 中 `ensure_replace_backend_consistency(&builder.tree.root, "")?` 之后加入：

```rust
    ensure_vfs_not_shadowed(&builder.tree.root, "", None)?;
```

在 `ensure_replace_backend_consistency` 之后新增：

```rust
/// VFS 规则作用在真实目录上；任何被 Overlay / Magic 以目录形式占用的祖先
/// 目录都会遮蔽其后代注入，因此必须在 plan 阶段显式报错。
///
/// 采用保守判定：只要祖先节点存在 Overlay/Magic 的目录来源（含 `.replace`），
/// 其下任何 Vfs 来源都视为被遮蔽。
fn ensure_vfs_not_shadowed(
    node: &MountNode,
    target: &str,
    ancestor_mount: Option<(Mode, &str)>,
) -> Result<()> {
    let current_target = if node.name.is_empty() {
        target.to_owned()
    } else if target.is_empty() {
        format!("/{}", node.name)
    } else {
        format!("{target}/{}", node.name)
    };

    if let (Some((mode, source)), Some(vfs_source)) = (
        ancestor_mount,
        node.sources.iter().find(|source| source.backend == Mode::Vfs),
    ) {
        return Err(Error::PlanConflict {
            target: current_target,
            first_backend: mode.as_str().to_owned(),
            first_source: source.to_owned(),
            second_backend: Mode::Vfs.as_str().to_owned(),
            second_source: format!("{}:{}", vfs_source.module_id, vfs_source.relative),
        });
    }

    let self_mount = node
        .sources
        .iter()
        .find(|source| {
            matches!(source.backend, Mode::Overlay | Mode::Magic)
                && (source.file_type == NodeFileType::Directory || source.replace)
        })
        .map(|source| (source.backend, source.relative.as_str()));

    let child_mount = self_mount.or(ancestor_mount);
    for child in node.children.values() {
        ensure_vfs_not_shadowed(child, &current_target, child_mount)?;
    }
    Ok(())
}
```

- [ ] **步骤 4：运行测试并确认其通过**

运行：`cargo test -p hybrid-mount vfs_file_ -- --nocapture`
预期：两个测试 PASS。

- [ ] **步骤 5：运行 planner 全量测试**

运行：`cargo test -p hybrid-mount plan:: -- --nocapture`
预期：全部 PASS。

- [ ] **步骤 6：提交**

```bash
git add src/plan/mod.rs
git commit -m "feat(plan): reject vfs targets shadowed by mounted ancestors"
```

---

## 任务 4：树→规则纯函数 `src/vfs/rule.rs`

**文件：**
- 新建：`src/vfs/mod.rs`
- 新建：`src/vfs/rule.rs`
- 新建：`src/vfs/rule_tests.rs`
- 修改：`src/main.rs:23-24`（`mod utils;` 之后新增 `mod vfs;`）

**接口：**
- 依赖输入：`MountTree`、`MountSource { module_id, relative, source_path, file_type, replace, backend }`、任务 1 的 `Mode::Vfs`。
- 对外产出：`pub enum VfsAction { Inject { virtual_path: String, real_path: PathBuf }, Whiteout { virtual_path: String } }`。
- 对外产出：`pub struct VfsRule { pub action: VfsAction, pub module_id: ModuleId }`。
- 对外产出：`pub fn build_vfs_rules(tree: &MountTree) -> Vec<VfsRule>`。

- [ ] **步骤 1：编写失败的测试**

新建 `src/vfs/rule_tests.rs`：

```rust
// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use crate::config::Mode;
use crate::mount_tree::{MountSource, MountTree, NodeFileType};
use crate::module_id::ModuleId;
use std::path::PathBuf;

fn source(module: &str, relative: &str, file_type: NodeFileType, backend: Mode) -> MountSource {
    MountSource {
        module_id: ModuleId::try_from(module).unwrap(),
        relative: relative.to_owned(),
        source_path: PathBuf::from(format!("/data/adb/modules/{module}/{relative}")),
        file_type,
        replace: false,
        backend,
    }
}

#[test]
fn file_and_symlink_become_inject_rules_in_tree_order() {
    let mut tree = MountTree::default();
    tree.insert(
        "/system/etc/hosts",
        source("m", "system/etc/hosts", NodeFileType::RegularFile, Mode::Vfs),
    );
    tree.insert(
        "/system/etc/link",
        source("m", "system/etc/link", NodeFileType::Symlink, Mode::Vfs),
    );

    let rules = build_vfs_rules(&tree);

    assert_eq!(
        rules,
        vec![
            VfsRule {
                action: VfsAction::Inject {
                    virtual_path: "/system/etc/hosts".to_owned(),
                    real_path: PathBuf::from("/data/adb/modules/m/system/etc/hosts"),
                },
                module_id: ModuleId::try_from("m").unwrap(),
            },
            VfsRule {
                action: VfsAction::Inject {
                    virtual_path: "/system/etc/link".to_owned(),
                    real_path: PathBuf::from("/data/adb/modules/m/system/etc/link"),
                },
                module_id: ModuleId::try_from("m").unwrap(),
            },
        ]
    );
}

#[test]
fn whiteout_becomes_whiteout_rule() {
    let mut tree = MountTree::default();
    tree.insert(
        "/system/etc/hidden.xml",
        source("m", "system/etc/hidden.xml", NodeFileType::Whiteout, Mode::Vfs),
    );

    assert_eq!(
        build_vfs_rules(&tree),
        vec![VfsRule {
            action: VfsAction::Whiteout {
                virtual_path: "/system/etc/hidden.xml".to_owned(),
            },
            module_id: ModuleId::try_from("m").unwrap(),
        }]
    );
}

#[test]
fn directory_and_other_backends_produce_no_rules() {
    let mut tree = MountTree::default();
    tree.insert(
        "/system/etc",
        source("m", "system/etc", NodeFileType::Directory, Mode::Vfs),
    );
    tree.insert(
        "/system/etc/hosts",
        source("m", "system/etc/hosts", NodeFileType::RegularFile, Mode::Overlay),
    );

    assert!(build_vfs_rules(&tree).is_empty());
}
```

- [ ] **步骤 2：运行测试并确认其失败**

运行：`cargo test -p hybrid-mount vfs::rule -- --nocapture`
预期：编译失败，提示找不到 `vfs` 模块（`src/main.rs` 未声明）。

---

- [ ] **步骤 3：编写最小实现**

新建 `src/vfs/mod.rs`：

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! NoMount 兼容的 VFS 后端（用户态）。

pub mod rule;
```

在 `src/main.rs` 的 `mod utils;` 之后加一行：

```rust
mod utils;
mod vfs;
```

新建 `src/vfs/rule.rs`：

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! 把共享挂载树里标注为 `vfs` 的节点映射为可下发的规则。
//! 目录只作为结构遍历，不产生规则；实际目录合并由内核虚拟拓扑完成。

use std::path::PathBuf;

use crate::config::Mode;
use crate::module_id::ModuleId;
use crate::mount_tree::{MountNode, MountTree, NodeFileType};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VfsAction {
    Inject { virtual_path: String, real_path: PathBuf },
    Whiteout { virtual_path: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VfsRule {
    pub action: VfsAction,
    pub module_id: ModuleId,
}

pub fn build_vfs_rules(tree: &MountTree) -> Vec<VfsRule> {
    let mut rules = Vec::new();
    collect_node(&tree.root, "", &mut rules);
    rules
}

fn collect_node(node: &MountNode, target: &str, out: &mut Vec<VfsRule>) {
    let current = if node.name.is_empty() {
        target.to_owned()
    } else if target.is_empty() {
        format!("/{}", node.name)
    } else {
        format!("{target}/{}", node.name)
    };

    for source in &node.sources {
        if source.backend != Mode::Vfs {
            continue;
        }
        let action = match source.file_type {
            NodeFileType::Directory => continue,
            NodeFileType::Whiteout => VfsAction::Whiteout {
                virtual_path: current.clone(),
            },
            NodeFileType::RegularFile | NodeFileType::Symlink => VfsAction::Inject {
                virtual_path: current.clone(),
                real_path: source.source_path.clone(),
            },
        };
        out.push(VfsRule {
            action,
            module_id: source.module_id.clone(),
        });
    }

    for child in node.children.values() {
        collect_node(child, &current, out);
    }
}

#[cfg(test)]
#[path = "rule_tests.rs"]
mod tests;
```

- [ ] **步骤 4：运行测试并确认其通过**

运行：`cargo test -p hybrid-mount vfs::rule -- --nocapture`
预期：3 个测试 PASS。

- [ ] **步骤 5：提交**

```bash
git add src/main.rs src/vfs/mod.rs src/vfs/rule.rs src/vfs/rule_tests.rs
git commit -m "feat(vfs): map mount tree to vfs rules"
```

---

## 任务 5：VFS 结构化错误变体

**文件：**
- 修改：`src/errors.rs:156-262`（`Error` 枚举）、`src/errors.rs:270-298`（`classify`）
- 测试：`src/errors_tests.rs`

**接口：**
- 对外产出：`Error::VfsProtocol { detail: String }`、`Error::VfsUnavailable { reason: String }`、`Error::VfsProviderConflict { detail: String }`、`Error::VfsUnsupportedVersion { found: String, supported: String }`。

- [ ] **步骤 1：编写失败的测试**

在 `src/errors_tests.rs` 末尾添加：

```rust
#[test]
fn vfs_errors_have_explicit_classes_and_messages() {
    let unsupported = crate::errors::Error::VfsUnsupportedVersion {
        found: "19".to_owned(),
        supported: "20".to_owned(),
    };
    assert_eq!(
        unsupported.classify(),
        crate::errors::ErrorClass::ManualRecovery
    );
    assert!(unsupported.to_string().contains("19"));

    let protocol = crate::errors::Error::VfsProtocol {
        detail: "buffer overflow".to_owned(),
    };
    assert_eq!(protocol.classify(), crate::errors::ErrorClass::Permanent);
}
```

- [ ] **步骤 2：运行测试并确认其失败**

运行：`cargo test -p hybrid-mount vfs_errors_have_explicit -- --nocapture`
预期：编译失败，提示 `no variant or associated item named 'VfsUnsupportedVersion'`。

- [ ] **步骤 3：编写最小实现**

在 `src/errors.rs` 的 `Error` 枚举中 `State(Box<ContextError>)` 之后插入：

```rust
    #[error("{0}")]
    Vfs(Box<ContextError>),

    #[error("VFS protocol error: {detail}")]
    VfsProtocol { detail: String },

    #[error("VFS kernel provider is unavailable: {reason}")]
    VfsUnavailable { reason: String },

    #[error("VFS provider conflict: {detail}")]
    VfsProviderConflict { detail: String },

    #[error("unsupported VFS protocol version {found:?} (supported: {supported})")]
    VfsUnsupportedVersion { found: String, supported: String },
```

在 `classify` 的 `Self::Mount(err) | Self::Storage(err) | Self::Lkm(err) | Self::State(err)` 分支改为：

```rust
            Self::Mount(err) | Self::Storage(err) | Self::Lkm(err) | Self::State(err) | Self::Vfs(err) => {
                err.source.classify()
            }
            Self::VfsProtocol { .. } => ErrorClass::Permanent,
            Self::VfsUnavailable { .. }
            | Self::VfsProviderConflict { .. }
            | Self::VfsUnsupportedVersion { .. } => ErrorClass::ManualRecovery,
```

- [ ] **步骤 4：运行测试并确认其通过**

运行：`cargo test -p hybrid-mount vfs_errors_have_explicit -- --nocapture`
预期：PASS。

- [ ] **步骤 5：提交**

```bash
git add src/errors.rs src/errors_tests.rs
git commit -m "feat(errors): add structured vfs error variants"
```

---

## 任务 6：`nm_payload` 字节级编解码 `src/vfs/protocol.rs`

**文件：**
- 新建：`src/vfs/protocol.rs`
- 新建：`src/vfs/protocol_tests.rs`
- 修改：`src/vfs/mod.rs`

**接口：**
- 依赖输入：任务 4 的 `VfsRule`/`VfsAction`；任务 5 的 `Error::VfsProtocol`。
- 对外产出：`pub enum NmCommand { GetVersion = 1, AddRule = 2, DelRule = 3, AddUid = 4, DelUid = 5, ClearAll = 6, ClearRules = 7, ClearUids = 8, GetList = 9, GetUids = 10 }`。
- 对外产出：`pub struct EncodedRule { pub flags: u32, pub virtual_path: Vec<u8>, pub real_path: Vec<u8> }`，方法 `record_len(&self) -> usize`、`write_into(&self, out: &mut Vec<u8>)`。
- 对外产出：`pub fn encode_rule(rule: &VfsRule) -> Result<EncodedRule>`、`build_payload(cmd, target_uid, buffer) -> Result<Vec<u8>>`、`build_add_rule_payloads(rules: &[EncodedRule], uid: u32) -> Result<Vec<Vec<u8>>>`、`parse_version(&[u8]) -> Result<String>`、`ensure_status(&[u8]) -> Result<()>`。

- [ ] **步骤 1：编写失败的测试**

新建 `src/vfs/protocol_tests.rs`：

```rust
// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use crate::module_id::ModuleId;
use crate::vfs::rule::{VfsAction, VfsRule};
use std::path::PathBuf;

#[test]
fn payload_has_exact_wire_layout() {
    let page = build_payload(NmCommand::GetVersion, 0, &[]).unwrap();
    assert_eq!(page.len(), PAYLOAD_LEN);
    assert_eq!(&page[0..8], &MAGIC.to_le_bytes());
    assert_eq!(u32::from_le_bytes(page[8..12].try_into().unwrap()), 1);
    assert_eq!(u32::from_le_bytes(page[12..16].try_into().unwrap()), 0);
    assert_eq!(i32::from_le_bytes(page[16..20].try_into().unwrap()), 0);
    assert_eq!(u32::from_le_bytes(page[20..24].try_into().unwrap()), 0);
    assert_eq!(u32::from_le_bytes(page[24..28].try_into().unwrap()), 0);
}

#[test]
fn add_rule_record_encodes_header_and_paths() {
    let rule = VfsRule {
        action: VfsAction::Inject {
            virtual_path: "/system/etc/hosts".to_owned(),
            real_path: PathBuf::from("/data/local/tmp/hosts"),
        },
        module_id: ModuleId::try_from("m").unwrap(),
    };
    let encoded = encode_rule(&rule).unwrap();
    assert_eq!(encoded.flags, 0);
    assert_eq!(
        encoded.record_len(),
        RULE_HEADER_LEN + "/system/etc/hosts".len() + "/data/local/tmp/hosts".len()
    );

    let pages = build_add_rule_payloads(&[encoded], 0).unwrap();
    assert_eq!(pages.len(), 1);
    let buffer = &pages[0][28..];
    assert_eq!(u32::from_le_bytes(buffer[0..4].try_into().unwrap()), 0);
    assert_eq!(u32::from_le_bytes(buffer[4..8].try_into().unwrap()), 0);
    assert_eq!(
        u16::from_le_bytes(buffer[8..10].try_into().unwrap()),
        "/system/etc/hosts".len() as u16
    );
    assert_eq!(
        u16::from_le_bytes(buffer[10..12].try_into().unwrap()),
        "/data/local/tmp/hosts".len() as u16
    );
    assert_eq!(&buffer[12..12 + 17], b"/system/etc/hosts");
    assert_eq!(
        u32::from_le_bytes(pages[0][8..12].try_into().unwrap()),
        NmCommand::AddRule as u32
    );
}

#[test]
fn whiteout_rule_sets_flag_and_empty_real_path() {
    let rule = VfsRule {
        action: VfsAction::Whiteout {
            virtual_path: "/system/etc/hidden.xml".to_owned(),
        },
        module_id: ModuleId::try_from("m").unwrap(),
    };
    let encoded = encode_rule(&rule).unwrap();
    assert_eq!(encoded.flags, FLAG_WHITEOUT);
    assert!(encoded.real_path.is_empty());
    assert_eq!(
        encoded.record_len(),
        RULE_HEADER_LEN + "/system/etc/hidden.xml".len()
    );
}

#[test]
fn batches_split_when_buffer_is_full() {
    let rule = VfsRule {
        action: VfsAction::Inject {
            virtual_path: "/system/etc/hosts".to_owned(),
            real_path: PathBuf::from("/data/local/tmp/hosts"),
        },
        module_id: ModuleId::try_from("m").unwrap(),
    };
    let repeated = vec![encode_rule(&rule).unwrap(); 200];
    let pages = build_add_rule_payloads(&repeated, 0).unwrap();
    assert!(pages.len() > 1);
    for page in &pages {
        assert_eq!(page.len(), PAYLOAD_LEN);
        let data_size = u32::from_le_bytes(page[24..28].try_into().unwrap()) as usize;
        assert!(data_size <= BUFFER_LEN);
    }
}

#[test]
fn parse_version_reads_buffer_and_len() {
    let mut page = build_payload(NmCommand::GetVersion, 0, &[]).unwrap();
    page[28..30].copy_from_slice(b"20");
    page[24..28].copy_from_slice(&2_u32.to_le_bytes());
    assert_eq!(parse_version(&page).unwrap(), "20");
}

#[test]
fn ensure_status_rejects_negative_kernel_status() {
    let mut page = build_payload(NmCommand::AddRule, 0, &[]).unwrap();
    page[16..20].copy_from_slice(&(-22_i32).to_le_bytes());
    let err = ensure_status(&page).unwrap_err();
    assert!(matches!(err, crate::errors::Error::VfsProtocol { .. }));
}
```

- [ ] **步骤 2：运行测试并确认其失败**

运行：`cargo test -p hybrid-mount vfs::protocol -- --nocapture`
预期：编译失败，提示找不到 `protocol` 模块。

---

- [ ] **步骤 3：编写最小实现**

新建 `src/vfs/protocol.rs`：

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! NoMount wire 协议：`add_key("nomount", "trigger", &payload_ptr)` 指向的
//! 单页 `nm_payload`。这里只做纯字节编解码，便于主机单测。

use crate::errors::{Error, Result};
use crate::vfs::rule::{VfsAction, VfsRule};

pub const MAGIC: u64 = 0x4E4F_4D4F_554E_54;
pub const PAYLOAD_LEN: usize = 4096;
pub const BUFFER_LEN: usize = 4068;
pub const RULE_HEADER_LEN: usize = 12;
pub const DEL_HEADER_LEN: usize = 6;

pub const FLAG_WHITEOUT: u32 = 1 << 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum NmCommand {
    GetVersion = 1,
    AddRule = 2,
    DelRule = 3,
    AddUid = 4,
    DelUid = 5,
    ClearAll = 6,
    ClearRules = 7,
    ClearUids = 8,
    GetList = 9,
    GetUids = 10,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodedRule {
    pub flags: u32,
    pub virtual_path: Vec<u8>,
    pub real_path: Vec<u8>,
}

impl EncodedRule {
    pub fn record_len(&self) -> usize {
        RULE_HEADER_LEN + self.virtual_path.len() + self.real_path.len()
    }

    pub fn write_into(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.flags.to_le_bytes());
        out.extend_from_slice(&0_u32.to_le_bytes());
        out.extend_from_slice(&(self.virtual_path.len() as u16).to_le_bytes());
        out.extend_from_slice(&(self.real_path.len() as u16).to_le_bytes());
        out.extend_from_slice(&self.virtual_path);
        out.extend_from_slice(&self.real_path);
    }
}

pub fn encode_rule(rule: &VfsRule) -> Result<EncodedRule> {
    match &rule.action {
        VfsAction::Inject {
            virtual_path,
            real_path,
        } => Ok(EncodedRule {
            flags: 0,
            virtual_path: virtual_path.as_bytes().to_vec(),
            real_path: real_path.to_string_lossy().as_bytes().to_vec(),
        }),
        VfsAction::Whiteout { virtual_path } => Ok(EncodedRule {
            flags: FLAG_WHITEOUT,
            virtual_path: virtual_path.as_bytes().to_vec(),
            real_path: Vec::new(),
        }),
    }
}

pub fn build_payload(cmd: NmCommand, target_uid: u32, buffer: &[u8]) -> Result<Vec<u8>> {
    if buffer.len() > BUFFER_LEN {
        return Err(Error::VfsProtocol {
            detail: format!("buffer {} exceeds {BUFFER_LEN}", buffer.len()),
        });
    }
    let mut page = vec![0_u8; PAYLOAD_LEN];
    page[0..8].copy_from_slice(&MAGIC.to_le_bytes());
    page[8..12].copy_from_slice(&(cmd as u32).to_le_bytes());
    page[12..16].copy_from_slice(&target_uid.to_le_bytes());
    page[24..28].copy_from_slice(&(buffer.len() as u32).to_le_bytes());
    page[28..28 + buffer.len()].copy_from_slice(buffer);
    Ok(page)
}

pub fn build_add_rule_payloads(rules: &[EncodedRule], uid: u32) -> Result<Vec<Vec<u8>>> {
    let mut payloads = Vec::new();
    let mut buffer: Vec<u8> = Vec::new();
    for rule in rules {
        if rule.record_len() > BUFFER_LEN {
            return Err(Error::VfsProtocol {
                detail: format!("single rule needs {} bytes", rule.record_len()),
            });
        }
        if buffer.len() + rule.record_len() > BUFFER_LEN {
            payloads.push(build_payload(NmCommand::AddRule, uid, &buffer)?);
            buffer.clear();
        }
        rule.write_into(&mut buffer);
    }
    if !buffer.is_empty() {
        payloads.push(build_payload(NmCommand::AddRule, uid, &buffer)?);
    }
    Ok(payloads)
}

pub fn ensure_status(payload: &[u8]) -> Result<()> {
    if payload.len() != PAYLOAD_LEN {
        return Err(Error::VfsProtocol {
            detail: format!("response {} bytes, expected {PAYLOAD_LEN}", payload.len()),
        });
    }
    let status = i32::from_le_bytes(payload[16..20].try_into().map_err(|_| {
        Error::VfsProtocol {
            detail: "status field is not four bytes".to_owned(),
        }
    })?);
    if status < 0 {
        return Err(Error::VfsProtocol {
            detail: format!("kernel returned status {status}"),
        });
    }
    Ok(())
}

pub fn parse_version(payload: &[u8]) -> Result<String> {
    ensure_status(payload)?;
    let len = u32::from_le_bytes(payload[24..28].try_into().map_err(|_| {
        Error::VfsProtocol {
            detail: "data_size field is not four bytes".to_owned(),
        }
    })?) as usize;
    if len > BUFFER_LEN {
        return Err(Error::VfsProtocol {
            detail: format!("version length {len} exceeds {BUFFER_LEN}"),
        });
    }
    let raw = &payload[28..28 + len];
    let text = std::str::from_utf8(raw).map_err(|_| Error::VfsProtocol {
        detail: "version is not valid utf-8".to_owned(),
    })?;
    Ok(text.trim().to_owned())
}

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;
```

在 `src/vfs/mod.rs` 增加 `protocol` 声明：

```rust
pub mod protocol;
pub mod rule;
```

- [ ] **步骤 4：运行测试并确认其通过**

运行：`cargo test -p hybrid-mount vfs::protocol -- --nocapture`
预期：6 个测试 PASS。

- [ ] **步骤 5：提交**

```bash
git add src/vfs/mod.rs src/vfs/protocol.rs src/vfs/protocol_tests.rs
git commit -m "feat(vfs): encode nomount wire payloads"
```

---

## 任务 7：keyring 发送通道 `src/vfs/sys.rs`

**文件：**
- 新建：`src/vfs/sys.rs`
- 修改：`Cargo.toml`（新增 `libc`）
- 修改：`src/vfs/mod.rs`

**接口：**
- 依赖输入：任务 6 的 `protocol::PAYLOAD_LEN`。
- 对外产出：`pub struct PageBuffer`，方法 `new() -> io::Result<Self>`、`as_mut_slice(&mut self) -> &mut [u8]`（长度恒为 4096）。
- 对外产出：`pub fn add_key(page: &mut PageBuffer) -> io::Result<()>`。

- [ ] **步骤 1：编写失败的测试**

在 `src/vfs/sys.rs` 末尾（实现之后）添加：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_buffer_exposes_one_payload() {
        let mut page = PageBuffer::new().unwrap();
        assert_eq!(
            page.as_mut_slice().len(),
            crate::vfs::protocol::PAYLOAD_LEN
        );
    }
}
```

- [ ] **步骤 2：运行测试并确认其失败**

运行：`cargo test -p hybrid-mount vfs::sys -- --nocapture`
预期：编译失败，提示找不到 `sys` 模块。

- [ ] **步骤 3：编写最小实现**

在 `Cargo.toml` 的 linux/android 目标依赖块追加 `libc`：

```toml
[target.'cfg(any(target_os = "linux", target_os = "android"))'.dependencies]
ksu = { git = "https://github.com/Tools-cx-app/ksu.git", version = "0.2.0" }
loopdev = { git = "https://github.com/Hybrid-Mount/loopdev.git", version = "0.5.0" }
procfs = "0.18"
rustix = { version = "1.1.4", features = ["fs", "mount", "thread"] }
libc = "0.2"
```

新建 `src/vfs/sys.rs`：

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! keyring 发送通道。内核 `nm_key_preparse` 要求 payload 位于一页的偏移 0，
//! 因此这里用 `mmap` 分配页对齐缓冲，绝不复用普通 `Vec` 堆内存。

#[cfg(any(target_os = "linux", target_os = "android"))]
pub use platform::{PageBuffer, add_key};

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub use stub::{PageBuffer, add_key};

#[cfg(any(target_os = "linux", target_os = "android"))]
mod platform {
    use std::io;
    use std::ptr::NonNull;

    pub struct PageBuffer {
        ptr: NonNull<u8>,
    }

    impl PageBuffer {
        pub fn new() -> io::Result<Self> {
            // SAFETY: mmap 返回的映射在本结构 Drop 前保持有效，长度固定为 PAYLOAD_LEN。
            let addr = unsafe {
                libc::mmap(
                    std::ptr::null_mut(),
                    crate::vfs::protocol::PAYLOAD_LEN,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                    -1,
                    0,
                )
            };
            if addr == libc::MAP_FAILED {
                return Err(io::Error::last_os_error());
            }
            let ptr = NonNull::new(addr.cast::<u8>())
                .ok_or_else(|| io::Error::other("mmap returned null"))?;
            Ok(Self { ptr })
        }

        pub fn as_mut_slice(&mut self) -> &mut [u8] {
            // SAFETY: 指针来自 mmap，长度固定，且 &mut self 保证独占访问。
            unsafe {
                std::slice::from_raw_parts_mut(self.ptr.as_ptr(), crate::vfs::protocol::PAYLOAD_LEN)
            }
        }
    }

    impl Drop for PageBuffer {
        fn drop(&mut self) {
            // SAFETY: 指针与长度来自同一 mmap。
            unsafe {
                libc::munmap(self.ptr.as_ptr().cast(), crate::vfs::protocol::PAYLOAD_LEN);
            }
        }
    }

    #[cfg(target_arch = "aarch64")]
    const SYS_ADD_KEY: libc::c_long = 217;
    #[cfg(target_arch = "arm")]
    const SYS_ADD_KEY: libc::c_long = 309;
    #[cfg(target_arch = "x86_64")]
    const SYS_ADD_KEY: libc::c_long = 248;

    /// 发送一页 payload。内核通过同一页回写 `status`，调用方随后自行解析。
    pub fn add_key(page: &mut PageBuffer) -> io::Result<()> {
        let ptr = page.ptr.as_ptr() as libc::c_ulong;
        // SAFETY: 变参 syscall，参数布局与内核 add_key(type, desc, &ptr, 8, -1) 一致。
        let ret = unsafe {
            libc::syscall(
                SYS_ADD_KEY,
                c"nomount".as_ptr(),
                c"trigger".as_ptr(),
                std::ptr::addr_of!(ptr),
                std::mem::size_of::<libc::c_ulong>(),
                -1_i32,
            )
        };
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
mod stub {
    use std::io;

    pub struct PageBuffer {
        bytes: Vec<u8>,
    }

    impl PageBuffer {
        pub fn new() -> io::Result<Self> {
            Ok(Self {
                bytes: vec![0_u8; crate::vfs::protocol::PAYLOAD_LEN],
            })
        }

        pub fn as_mut_slice(&mut self) -> &mut [u8] {
            &mut self.bytes
        }
    }

    pub fn add_key(_page: &mut PageBuffer) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "nomount keyring is linux/android only",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_buffer_exposes_one_payload() {
        let mut page = PageBuffer::new().unwrap();
        assert_eq!(
            page.as_mut_slice().len(),
            crate::vfs::protocol::PAYLOAD_LEN
        );
    }
}
```

在 `src/vfs/mod.rs` 增加 `sys` 声明：

```rust
pub mod protocol;
pub mod rule;
pub mod sys;
```

- [ ] **步骤 4：运行测试并确认其通过**

运行：`cargo test -p hybrid-mount vfs::sys -- --nocapture`
预期：PASS（host 使用 stub）。
再运行：`cargo check -p hybrid-mount --target aarch64-linux-android`
预期：编译通过（使用 platform 实现）。

- [ ] **步骤 5：提交**

```bash
git add Cargo.toml Cargo.lock src/vfs/mod.rs src/vfs/sys.rs
git commit -m "feat(vfs): add keyring page buffer and add_key sender"
```

---

## 任务 8：Provider 选择与二选一互斥 `src/vfs/backend.rs`

**文件：**
- 新建：`src/vfs/backend.rs`
- 新建：`src/vfs/backend_tests.rs`
- 修改：`src/vfs/mod.rs`

**接口：**
- 依赖输入：任务 6 的 `protocol`；任务 7 的 `sys::{PageBuffer, add_key}`；任务 5 的 VFS 错误。
- 对外产出：`pub enum VfsProvider { Nomount, Hm }`，`as_str()`。
- 对外产出：`pub const SUPPORTED_VERSIONS: &[&str] = &["20"];`。
- 对外产出：`pub trait VfsKernel { fn version(&mut self) -> Result<String>; fn apply_rules(&mut self, rules: &[EncodedRule]) -> Result<()>; fn add_uids(&mut self, uids: &[u32]) -> Result<()>; fn clear_rules(&mut self) -> Result<()>; }`。
- 对外产出：`pub trait LkmLoader { fn load_hm_vfs(&self) -> Result<()>; }`。
- 对外产出：`pub struct KeyringKernel`（实现 `VfsKernel`）。
- 对外产出：`pub fn select_provider(kernel: &mut dyn VfsKernel, loader: &dyn LkmLoader, supported: &[&str], hm_loaded: bool) -> Result<Option<VfsProvider>>`。

- [ ] **步骤 1：编写失败的测试**

新建 `src/vfs/backend_tests.rs`：

```rust
// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use crate::errors::Error;
use crate::vfs::protocol::EncodedRule;
use std::cell::Cell;

struct MockKernel<'a> {
    before: Option<&'static str>,
    after: Option<&'static str>,
    loaded: &'a Cell<bool>,
    applied: usize,
}

impl VfsKernel for MockKernel<'_> {
    fn version(&mut self) -> Result<String> {
        let value = if self.loaded.get() { self.after } else { self.before };
        value.map(str::to_owned).ok_or(Error::VfsUnavailable {
            reason: "absent".to_owned(),
        })
    }

    fn apply_rules(&mut self, rules: &[EncodedRule]) -> Result<()> {
        self.applied += rules.len();
        Ok(())
    }

    fn add_uids(&mut self, _uids: &[u32]) -> Result<()> {
        Ok(())
    }

    fn clear_rules(&mut self) -> Result<()> {
        Ok(())
    }
}

struct MockLoader<'a> {
    loaded: &'a Cell<bool>,
    calls: Cell<usize>,
}

impl LkmLoader for MockLoader<'_> {
    fn load_hm_vfs(&self) -> Result<()> {
        self.calls.set(self.calls.get() + 1);
        self.loaded.set(true);
        Ok(())
    }
}

#[test]
fn existing_supported_provider_is_adopted_without_loading() {
    let loaded = Cell::new(true);
    let mut kernel = MockKernel {
        before: Some("20"),
        after: Some("20"),
        loaded: &loaded,
        applied: 0,
    };
    let loader = MockLoader {
        loaded: &loaded,
        calls: Cell::new(0),
    };

    let selected = select_provider(&mut kernel, &loader, SUPPORTED_VERSIONS, false).unwrap();

    assert_eq!(selected, Some(VfsProvider::Nomount));
    assert_eq!(loader.calls.get(), 0);
}

#[test]
fn missing_provider_loads_hm_lkm_then_reports_hm() {
    let loaded = Cell::new(false);
    let mut kernel = MockKernel {
        before: None,
        after: Some("20"),
        loaded: &loaded,
        applied: 0,
    };
    let loader = MockLoader {
        loaded: &loaded,
        calls: Cell::new(0),
    };

    let selected = select_provider(&mut kernel, &loader, SUPPORTED_VERSIONS, false).unwrap();

    assert_eq!(selected, Some(VfsProvider::Hm));
    assert_eq!(loader.calls.get(), 1);
}

#[test]
fn missing_provider_stays_unavailable_when_lkm_does_not_help() {
    let loaded = Cell::new(false);
    let mut kernel = MockKernel {
        before: None,
        after: None,
        loaded: &loaded,
        applied: 0,
    };
    let loader = MockLoader {
        loaded: &loaded,
        calls: Cell::new(0),
    };

    let selected = select_provider(&mut kernel, &loader, SUPPORTED_VERSIONS, false).unwrap();

    assert_eq!(selected, None);
    assert_eq!(loader.calls.get(), 1);
}

#[test]
fn unsupported_existing_version_is_rejected_without_loading() {
    let loaded = Cell::new(false);
    let mut kernel = MockKernel {
        before: Some("19"),
        after: Some("20"),
        loaded: &loaded,
        applied: 0,
    };
    let loader = MockLoader {
        loaded: &loaded,
        calls: Cell::new(0),
    };

    let err = select_provider(&mut kernel, &loader, SUPPORTED_VERSIONS, false).unwrap_err();

    assert!(matches!(err, Error::VfsUnsupportedVersion { .. }));
    assert_eq!(loader.calls.get(), 0);
}
```

- [ ] **步骤 2：运行测试并确认其失败**

运行：`cargo test -p hybrid-mount vfs::backend -- --nocapture`
预期：编译失败，提示找不到 `backend` 模块。

---

- [ ] **步骤 3：编写最小实现**

新建 `src/vfs/backend.rs`：

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! 唯一活动的 VFS 内核 Provider：K1（NoMount）或 K2（HM 自有），二选一。
//! 选择结果在本次启动内固定，禁止热切换。

use crate::errors::{Error, Result};
use crate::vfs::protocol::{self, EncodedRule, NmCommand};
use crate::vfs::sys::{self, PageBuffer};

pub const SUPPORTED_VERSIONS: &[&str] = &["20"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VfsProvider {
    Nomount,
    Hm,
}

impl VfsProvider {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Nomount => "nomount",
            Self::Hm => "hm",
        }
    }
}

pub trait VfsKernel {
    fn version(&mut self) -> Result<String>;
    fn apply_rules(&mut self, rules: &[EncodedRule]) -> Result<()>;
    fn add_uids(&mut self, uids: &[u32]) -> Result<()>;
    fn clear_rules(&mut self) -> Result<()>;
}

pub trait LkmLoader {
    fn load_hm_vfs(&self) -> Result<()>;
}

pub struct KeyringKernel {
    page: PageBuffer,
}

impl KeyringKernel {
    pub fn new() -> Result<Self> {
        let page = PageBuffer::new().map_err(Error::Io)?;
        Ok(Self { page })
    }

    fn exchange(&mut self, request: &[u8]) -> Result<Vec<u8>> {
        self.page.as_mut_slice().copy_from_slice(request);
        sys::add_key(&mut self.page).map_err(Error::Io)?;
        Ok(self.page.as_mut_slice().to_vec())
    }
}

impl VfsKernel for KeyringKernel {
    fn version(&mut self) -> Result<String> {
        let request = protocol::build_payload(NmCommand::GetVersion, 0, &[])?;
        let response = self.exchange(&request)?;
        protocol::parse_version(&response)
    }

    fn apply_rules(&mut self, rules: &[EncodedRule]) -> Result<()> {
        for page in protocol::build_add_rule_payloads(rules, 0)? {
            let response = self.exchange(&page)?;
            protocol::ensure_status(&response)?;
        }
        Ok(())
    }

    fn add_uids(&mut self, uids: &[u32]) -> Result<()> {
        for uid in uids {
            let request = protocol::build_payload(NmCommand::AddUid, *uid, &[])?;
            let response = self.exchange(&request)?;
            protocol::ensure_status(&response)?;
        }
        Ok(())
    }

    fn clear_rules(&mut self) -> Result<()> {
        let request = protocol::build_payload(NmCommand::ClearRules, 0, &[])?;
        let response = self.exchange(&request)?;
        protocol::ensure_status(&response)?;
        Ok(())
    }
}

/// 选择唯一活动的 Provider。
///
/// 1. 已有可响应且版本受支持的 Provider：直接采用，不加载任何模块；
/// 2. 否则尝试加载 HM 自有 VFS LKM，再重新探测；
/// 3. 仍不可用返回 `Ok(None)`，由调用方决定降级或失败。
pub fn select_provider(
    kernel: &mut dyn VfsKernel,
    loader: &dyn LkmLoader,
    supported: &[&str],
    hm_loaded: bool,
) -> Result<Option<VfsProvider>> {
    match kernel.version() {
        Ok(found) if supported.contains(&found.as_str()) => {
            return Ok(Some(if hm_loaded {
                VfsProvider::Hm
            } else {
                VfsProvider::Nomount
            }));
        }
        Ok(found) => {
            return Err(Error::VfsUnsupportedVersion {
                found,
                supported: supported.join(","),
            });
        }
        Err(_) => {}
    }

    loader.load_hm_vfs()?;

    match kernel.version() {
        Ok(found) if supported.contains(&found.as_str()) => Ok(Some(VfsProvider::Hm)),
        Ok(found) => Err(Error::VfsUnsupportedVersion {
            found,
            supported: supported.join(","),
        }),
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
#[path = "backend_tests.rs"]
mod tests;
```

在 `src/vfs/mod.rs` 增加 `backend` 声明：

```rust
pub mod backend;
pub mod protocol;
pub mod rule;
pub mod sys;
```

- [ ] **步骤 4：运行测试并确认其通过**

运行：`cargo test -p hybrid-mount vfs::backend -- --nocapture`
预期：4 个测试 PASS。

- [ ] **步骤 5：提交**

```bash
git add src/vfs/mod.rs src/vfs/backend.rs src/vfs/backend_tests.rs
git commit -m "feat(vfs): select a single kernel provider"
```

---

## 任务 9：状态快照暴露 vfs 字段

**文件：**
- 修改：`src/state.rs:63-68`（`ModeStats`）、`src/state.rs:116-153`（`RunState`）、`src/state.rs:226-264`（`RunState::new`）、`src/state.rs:268-308`（`from_plan`）、`src/state.rs:369-395`（`app_modules`）
- 测试：`src/state.rs` 内 `mod tests`（若无则新建 `src/state_tests.rs` 并按现有 `#[path]` 约定挂载）

**接口：**
- 依赖输入：任务 2 的 `MountPlan.vfs_module_ids`。
- 对外产出：`ModeStats.vfs: usize`；`RunState.vfs_modules: Vec<String>`、`RunState.vfs_active_mounts: Vec<String>`、`RunState.vfs_provider: Option<String>`。

- [ ] **步骤 1：编写失败的测试**

在 `src/state.rs` 的测试模块添加：

```rust
#[test]
fn mode_stats_exposes_vfs_counter() {
    let json = serde_json::to_string(&ModeStats::default()).unwrap();
    assert!(json.contains("\"vfs\":0"), "json was {json}");
}

#[test]
fn run_state_defaults_vfs_provider_to_none() {
    let state = RunState::default();
    assert!(state.vfs_provider.is_none());
    assert!(state.vfs_modules.is_empty());
    assert!(state.vfs_active_mounts.is_empty());
}
```

- [ ] **步骤 2：运行测试并确认其失败**

运行：`cargo test -p hybrid-mount vfs_counter -- --nocapture`
预期：编译失败，提示 `no field 'vfs' on type 'ModeStats'`。

- [ ] **步骤 3：编写最小实现**

`ModeStats` 增加字段：

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ModeStats {
    pub overlayfs: usize,
    pub magicmount: usize,
    pub vfs: usize,
}
```

`RunState` 的 `magic_active_mounts` 之后增加：

```rust
    /// VFS 注入模块与成功目标；VFS 不是真实挂载，不进入 `active_mounts`。
    pub vfs_modules: Vec<String>,
    pub vfs_active_mounts: Vec<String>,
    /// 本次启动实际绑定的 Provider（`nomount` / `hm`）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vfs_provider: Option<String>,
```

在 `RunState::new` 的 `Self { .. }` 中 `magic_active_mounts,` 之后增加：

```rust
            vfs_modules: Vec::new(),
            vfs_active_mounts: Vec::new(),
            vfs_provider: None,
```

在 `from_plan` 的 `ModeStats { .. }` 增加 `vfs` 并回填模块：

```rust
            ModeStats {
                overlayfs: plan.overlay_module_ids.len(),
                magicmount: plan.magic_module_ids.len(),
                vfs: plan.vfs_module_ids.len(),
            },
        );
        state.vfs_modules = plan.vfs_module_ids.iter().map(ModuleId::to_string).collect();
```

在 `app_modules` 的后端判定链中，`plan.magic_module_ids.contains(&module.id)` 分支之后增加：

```rust
            } else if plan.vfs_module_ids.contains(&module.id) {
                Mode::Vfs
```

- [ ] **步骤 4：运行测试并确认其通过**

运行：`cargo test -p hybrid-mount state -- --nocapture`
预期：全部 PASS。

- [ ] **步骤 5：提交**

```bash
git add src/state.rs
git commit -m "feat(state): expose vfs modules and provider"
```

---

## 任务 10：应用规则与统计 `src/vfs/exec.rs`

**文件：**
- 新建：`src/vfs/exec.rs`
- 新建：`src/vfs/exec_tests.rs`
- 修改：`src/vfs/mod.rs`

**接口：**
- 依赖输入：任务 4 的 `build_vfs_rules`；任务 6 的 `encode_rule`；任务 8 的 `VfsKernel`；`MountPlan.vfs_module_ids`。
- 对外产出：`pub struct VfsExecStats { pub mounted_module_ids: Vec<String>, pub active_targets: Vec<String>, pub injected: usize, pub whiteouts: usize }`（`#[derive(Default)]`）。
- 对外产出：`pub fn apply_plan(kernel: &mut dyn VfsKernel, plan: &MountPlan, uids: &[u32]) -> Result<VfsExecStats>`。

- [ ] **步骤 1：编写失败的测试**

新建 `src/vfs/exec_tests.rs`：

```rust
// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use crate::config::Mode;
use crate::errors::Result;
use crate::mount_tree::{MountSource, MountTree, NodeFileType};
use crate::module_id::ModuleId;
use crate::plan::MountPlan;
use crate::vfs::backend::VfsKernel;
use crate::vfs::protocol::EncodedRule;
use crate::vfs::rule::VfsAction;
use std::path::PathBuf;

#[derive(Default)]
struct RecordingKernel {
    applied: usize,
    uids_added: Vec<u32>,
}

impl VfsKernel for RecordingKernel {
    fn version(&mut self) -> Result<String> {
        Ok("20".to_owned())
    }

    fn apply_rules(&mut self, rules: &[EncodedRule]) -> Result<()> {
        self.applied += rules.len();
        Ok(())
    }

    fn add_uids(&mut self, uids: &[u32]) -> Result<()> {
        self.uids_added = uids.to_vec();
        Ok(())
    }

    fn clear_rules(&mut self) -> Result<()> {
        Ok(())
    }
}

fn source(module: &str, relative: &str, file_type: NodeFileType) -> MountSource {
    MountSource {
        module_id: ModuleId::try_from(module).unwrap(),
        relative: relative.to_owned(),
        source_path: PathBuf::from(format!("/data/adb/modules/{module}/{relative}")),
        file_type,
        replace: false,
        backend: Mode::Vfs,
    }
}

#[test]
fn apply_plan_sends_rules_uids_and_reports_counts() {
    let mut tree = MountTree::default();
    tree.insert(
        "/system/etc/hosts",
        source("m", "system/etc/hosts", NodeFileType::RegularFile),
    );
    tree.insert(
        "/system/etc/hidden.xml",
        source("m", "system/etc/hidden.xml", NodeFileType::Whiteout),
    );

    let plan = MountPlan {
        tree,
        vfs_module_ids: vec![ModuleId::try_from("m").unwrap()],
        ..MountPlan::default()
    };
    let mut kernel = RecordingKernel::default();

    let stats = apply_plan(&mut kernel, &plan, &[1000]).unwrap();

    assert_eq!(stats.injected, 1);
    assert_eq!(stats.whiteouts, 1);
    assert_eq!(stats.mounted_module_ids, vec!["m".to_owned()]);
    assert_eq!(
        stats.active_targets,
        vec![
            "/system/etc/hidden.xml".to_owned(),
            "/system/etc/hosts".to_owned()
        ]
    );
    assert_eq!(kernel.applied, 2);
    assert_eq!(kernel.uids_added, vec![1000]);
}

#[test]
fn apply_plan_skips_uid_call_when_no_isolated_uids() {
    let mut tree = MountTree::default();
    tree.insert(
        "/system/etc/hosts",
        source("m", "system/etc/hosts", NodeFileType::RegularFile),
    );
    let plan = MountPlan {
        tree,
        vfs_module_ids: vec![ModuleId::try_from("m").unwrap()],
        ..MountPlan::default()
    };
    let mut kernel = RecordingKernel::default();

    apply_plan(&mut kernel, &plan, &[]).unwrap();

    assert!(kernel.uids_added.is_empty());
}
```

- [ ] **步骤 2：运行测试并确认其失败**

运行：`cargo test -p hybrid-mount vfs::exec -- --nocapture`
预期：编译失败，提示找不到 `exec` 模块。

- [ ] **步骤 3：编写最小实现**

新建 `src/vfs/exec.rs`：

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! 把规划结果应用到当前绑定的 Provider，并汇总统计。
//! 回滚由流水线统一负责（`clear_rules`）。

use crate::errors::Result;
use crate::module_id::ModuleId;
use crate::plan::MountPlan;
use crate::vfs::backend::VfsKernel;
use crate::vfs::protocol::encode_rule;
use crate::vfs::rule::{VfsAction, build_vfs_rules};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VfsExecStats {
    pub mounted_module_ids: Vec<String>,
    pub active_targets: Vec<String>,
    pub injected: usize,
    pub whiteouts: usize,
}

pub fn apply_plan(
    kernel: &mut dyn VfsKernel,
    plan: &MountPlan,
    uids: &[u32],
) -> Result<VfsExecStats> {
    let rules = build_vfs_rules(&plan.tree);
    let mut encoded = Vec::with_capacity(rules.len());
    let mut active_targets = Vec::with_capacity(rules.len());
    let mut injected = 0;
    let mut whiteouts = 0;
    for rule in &rules {
        match &rule.action {
            VfsAction::Inject { virtual_path, .. } => {
                injected += 1;
                active_targets.push(virtual_path.clone());
            }
            VfsAction::Whiteout { virtual_path } => {
                whiteouts += 1;
                active_targets.push(virtual_path.clone());
            }
        }
        encoded.push(encode_rule(rule)?);
    }

    kernel.apply_rules(&encoded)?;
    if !uids.is_empty() {
        kernel.add_uids(uids)?;
    }

    Ok(VfsExecStats {
        mounted_module_ids: plan
            .vfs_module_ids
            .iter()
            .map(ModuleId::to_string)
            .collect(),
        active_targets,
        injected,
        whiteouts,
    })
}

#[cfg(test)]
#[path = "exec_tests.rs"]
mod tests;
```

在 `src/vfs/mod.rs` 增加 `exec` 声明：

```rust
pub mod backend;
pub mod exec;
pub mod protocol;
pub mod rule;
pub mod sys;
```

- [ ] **步骤 4：运行测试并确认其通过**

运行：`cargo test -p hybrid-mount vfs::exec -- --nocapture`
预期：2 个测试 PASS。

- [ ] **步骤 5：提交**

```bash
git add src/vfs/mod.rs src/vfs/exec.rs src/vfs/exec_tests.rs
git commit -m "feat(vfs): apply plan rules and report stats"
```

---

## 任务 11：流水线接入 vfs 阶段（Android 目标）

**文件：**
- 修改：`src/defs.rs`（新增 boot guard 路径）
- 修改：`src/pipeline.rs:303`（结果类型）、`src/pipeline.rs:306-440`（`execute_mount_phases`）、`src/pipeline.rs:640-860`（`run_mount_pipeline_impl` 解构与状态装配）
- 测试：`src/pipeline.rs` 内 `mod tests`（沿用现有 host 可跑用例）

**接口：**
- 依赖输入：任务 8 `select_provider`/`KeyringKernel`/`SUPPORTED_VERSIONS`；任务 10 `apply_plan`/`VfsExecStats`；任务 9 的 `RunState` 字段。
- 对外产出：`MountExecutionResult` 追加 `VfsExecStats`；`defs::VFS_BOOT_GUARD_PATH`。

> 本任务代码全部位于 `#[cfg(any(target_os = "linux", target_os = "android"))]` 分支，宿主机测试不覆盖；验证以 Android 目标 `cargo check` 与现有 host 用例为准。

- [ ] **步骤 1：新增 boot guard 路径常量与测试**

在 `src/defs.rs` 的 `LKM_BOOT_GUARD_PATH` 之后增加：

```rust
/// VFS 后端启动熔断标记：本次启动下发规则前写入，成功后清除。
/// 硬崩溃遗留该标记时，下次启动跳过 vfs 后端。
pub const VFS_BOOT_GUARD_PATH: &str = "/data/adb/hybrid-mount/vfs_boot_guard";
```

在 `src/defs.rs` 的 `mod tests` 增加：

```rust
#[test]
fn vfs_boot_guard_lives_under_run_directory() {
    assert!(VFS_BOOT_GUARD_PATH.starts_with("/data/adb/hybrid-mount/"));
    assert_ne!(VFS_BOOT_GUARD_PATH, LKM_BOOT_GUARD_PATH);
}
```

- [ ] **步骤 2：运行新测试**

运行：`cargo test -p hybrid-mount vfs_boot_guard_lives -- --nocapture`
预期：PASS。

- [ ] **步骤 3：接入流水线**

在 `src/pipeline.rs` 顶部 `cfg` 导入块增加：

```rust
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::vfs::backend::{
    KeyringKernel, LkmLoader, SUPPORTED_VERSIONS, VfsKernel, select_provider,
};
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::vfs::exec::VfsExecStats;
use std::cell::RefCell;
use std::rc::Rc;
```

把 `src/pipeline.rs:303` 的类型改为：

```rust
type MountExecutionResult = (
    usize,
    usize,
    Vec<String>,
    exec::MagicMountStats,
    crate::vfs::exec::VfsExecStats,
);
```

在 `mount_magic_phase` 之前新增：

```rust
/// K2（HM 自有 VFS 内核实现）的加载由内核子系统计划接入；
/// 用户态此阶段只支持设备已有的 K1 Provider。
struct PendingKernelLoader;

impl LkmLoader for PendingKernelLoader {
    fn load_hm_vfs(&self) -> Result<()> {
        log::warn!(
            "hm vfs kernel implementation is not installed; only an existing nomount provider can be used"
        );
        Ok(())
    }
}

fn apply_vfs_phase(
    config: &Config,
    plan: &MountPlan,
    state: &mut RunState,
    transaction: &mut crate::sys::transaction::MountTransaction<'_>,
) -> Result<VfsExecStats> {
    if plan.vfs_module_ids.is_empty() {
        return Ok(VfsExecStats::default());
    }
    let guard = Path::new(defs::VFS_BOOT_GUARD_PATH);
    if guard.exists() {
        log::warn!("vfs boot guard present; skipping vfs backend this boot");
        return Ok(VfsExecStats::default());
    }
    crate::sys::fs::atomic_write(guard, b"1").map_err(|err| Error::Vfs {
        source: Box::new(crate::errors::ContextError::new(
            "write vfs boot guard",
            Some(guard.to_path_buf()),
            err,
        )),
    })?;

    let mut kernel = KeyringKernel::new()?;
    let loader = PendingKernelLoader;
    let Some(provider) = select_provider(&mut kernel, &loader, SUPPORTED_VERSIONS, false)? else {
        if config.vfs_strict {
            return Err(Error::VfsUnavailable {
                reason: "no supported VFS kernel provider and vfs_strict is enabled".to_owned(),
            });
        }
        log::warn!("vfs backend unavailable; vfs modules are skipped this boot");
        return Ok(VfsExecStats::default());
    };

    let outcome = crate::vfs::exec::apply_plan(&mut kernel, plan, &config.vfs_isolate_uids);
    if let Err(err) = fs::remove_file(guard) {
        log::warn!("clear vfs boot guard failed: {err}");
    }
    let stats = outcome?;

    state.vfs_provider = Some(provider.as_str().to_owned());
    let shared = Rc::new(RefCell::new(kernel));
    let rollback = Rc::clone(&shared);
    transaction.register_rollback_only("vfs_rules", move || {
        if let Ok(mut kernel) = rollback.try_borrow_mut() {
            kernel.clear_rules()?;
        }
        Ok(())
    });
    log::info!(
        "vfs phase complete: provider={}, injected={}, whiteouts={}",
        provider.as_str(),
        stats.injected,
        stats.whiteouts
    );
    Ok(stats)
}
```

在 `execute_mount_phases` 的 `magic_phase.finish();` 之后、`crate::utils::ksu::commit_unmount_list()?;` 之前插入：

```rust
    let vfs_phase = PhaseTimer::start("vfs");
    let vfs_stats = apply_vfs_phase(config, plan, state, transaction)?;
    vfs_phase.finish();
```

并把该函数末尾的返回值改为：

```rust
    Ok((
        overlay_dir_mounts,
        shallow_overlay_mounts,
        active_mounts,
        magic_stats,
        vfs_stats,
    ))
```

在 `run_mount_pipeline_impl` 中更新解构（`src/pipeline.rs:764`）：

```rust
    let (overlay_dir_mounts, shallow_overlay_mounts, active_mounts, magic_stats, vfs_stats) =
```

在 `mounted_module_ids.extend(magic_stats.mounted_module_ids.iter().cloned());` 之后增加：

```rust
    mounted_module_ids.extend(vfs_stats.mounted_module_ids.iter().cloned());
```

在 `state.magic_active_mounts = confirmed_magic_targets;` 之后增加：

```rust
    state.vfs_active_mounts = vfs_stats.active_targets.clone();
```

- [ ] **步骤 4：验证 Android 目标编译与宿主机门禁**

运行：`cargo check -p hybrid-mount --target aarch64-linux-android`
预期：编译通过（如缺目标先 `rustup target add aarch64-linux-android --toolchain nightly`）。
运行：`cargo check -p hybrid-mount --target armv7-linux-androideabi`
预期：编译通过，无 32 位宽度告警。
运行：`cargo clippy --workspace --all-targets -- -D warnings`
预期：无告警。
运行：`cargo test --workspace`
预期：全部 PASS。

- [ ] **步骤 5：提交**

```bash
git add src/defs.rs src/pipeline.rs
git commit -m "feat(pipeline): run vfs backend after magic mount"
```

---

## 任务 12：新增 `vfs_strict` 与 `vfs_isolate_uids` 配置

**文件：**
- 修改：`src/config.rs:83-126`（`Config`）、`src/config.rs:128-143`（`Default`）
- 修改：`module/config.toml`
- 测试：`src/config_tests.rs`

**接口：**
- 对外产出：`Config.vfs_strict: bool`（默认 `false`）、`Config.vfs_isolate_uids: Vec<u32>`（默认空）。
- 依赖输入：任务 11 中 `apply_vfs_phase` 引用的两个字段。

- [ ] **步骤 1：编写失败的测试**

在 `src/config_tests.rs` 末尾添加：

```rust
#[test]
fn vfs_strict_and_isolated_uids_parse_and_roundtrip() {
    let config = crate::config::Config::from_toml(
        "vfs_strict = true\nvfs_isolate_uids = [1000, 1001]\n",
    )
    .unwrap();
    assert!(config.vfs_strict);
    assert_eq!(config.vfs_isolate_uids, vec![1000, 1001]);
    assert!(config.to_toml().unwrap().contains("vfs_isolate_uids = ["));
}

#[test]
fn vfs_fields_default_off() {
    let config = crate::config::Config::from_toml("").unwrap();
    assert!(!config.vfs_strict);
    assert!(config.vfs_isolate_uids.is_empty());
}
```

- [ ] **步骤 2：运行测试并确认其失败**

运行：`cargo test -p hybrid-mount vfs_fields_default_off -- --nocapture`
预期：编译失败，提示 `no field 'vfs_strict'`。

- [ ] **步骤 3：编写最小实现**

在 `src/config.rs` 的 `Config` 中 `default_mode` 之后增加：

```rust
    /// VFS 后端不可用时是否直接失败（`true`）或降级（`false`）。
    #[serde(default)]
    pub vfs_strict: bool,

    /// 需要隔离（看到原生文件系统）的 UID 列表，下发给 VFS Provider。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vfs_isolate_uids: Vec<u32>,
```

在 `impl Default for Config` 的 `Self { .. }` 中 `default_mode: Mode::default(),` 之后增加：

```rust
            vfs_strict: false,
            vfs_isolate_uids: Vec::new(),
```

在 `module/config.toml` 的 `default_mode` 行后追加：

```toml
# default_mode 也接受 "vfs"（NoMount 兼容的 VFS 注入后端）。
# vfs_strict = false        # true 时 VFS 不可用即启动失败
# vfs_isolate_uids = []     # 这些 UID 将看到原生文件系统
```

- [ ] **步骤 4：运行测试并确认其通过**

运行：`cargo test -p hybrid-mount vfs_ -- --nocapture`
预期：全部 PASS。

- [ ] **步骤 5：提交**

```bash
git add src/config.rs src/config_tests.rs module/config.toml
git commit -m "feat(config): add vfs strict and uid isolation options"
```

---

## 任务 13：WebUI 暴露 vfs 模式与状态

**文件：**
- 修改：`webui/src/lib/types.ts:3`、`webui/src/lib/types.ts:49-50`、`webui/src/lib/types.ts` 的 `RunState`
- 修改：`webui/src/lib/api.ts:51-56`、`webui/src/lib/api.ts:221-224`
- 修改：`webui/src/ui/md3/pages/config.vue:14`、`webui/src/ui/md3/pages/modules.vue:15`
- 修改：`webui/src/ui/miuix/pages/config.vue:30`、`webui/src/ui/miuix/pages/modules.vue:29`
- 修改：`webui/src/ui/md3/pages/status.vue:22-23`、`webui/src/ui/miuix/pages/status.vue:27`
- 修改：`webui/src/locales/*.json`

**接口：**
- 依赖输入：任务 9 的 `ModeStats.vfs`、`RunState.vfs_modules/vfs_active_mounts/vfs_provider`；任务 11 的 CLI JSON。

- [ ] **步骤 1：编写失败的测试**

在 `webui/src/lib/config.test.ts` 增加：

```ts
it("accepts vfs as a default mode", () => {
  expect(normalizeConfigPayload({ default_mode: "vfs" }).default_mode).toBe("vfs");
});
```

- [ ] **步骤 2：运行测试并确认其失败**

运行：`cd webui && pnpm test -- --run config`
预期：FAIL —— `normalizeConfigPayload` 回退为非 `vfs` 值或类型错误。

- [ ] **步骤 3：编写最小实现**

`webui/src/lib/types.ts`：

```ts
export type MountMode = "overlay" | "magic" | "vfs" | "ignore";

export interface ModeStats {
  overlayfs: number;
  magicmount: number;
  vfs: number;
}
```

并在 `RunState` 增加：

```ts
  vfs_modules: string[];
  vfs_active_mounts: string[];
  vfs_provider?: string | null;
```

`webui/src/lib/api.ts`：

```ts
const isMountMode = (value: unknown): value is MountMode =>
  value === "overlay" || value === "magic" || value === "vfs" || value === "ignore";

const isDefaultMountMode = (value: unknown): value is DefaultMountMode =>
  value === "overlay" || value === "magic" || value === "vfs";
```

并在 `mode_stats` 解析处增加：

```ts
      vfs: Number((payload.mode_stats as Record<string, unknown>)?.vfs ?? 0),
```

四处模式列表加入 `"vfs"`：

```ts
const modeOptions: DefaultMountMode[] = ["overlay", "magic", "vfs"];
const modeOptions: MountMode[] = ["overlay", "magic", "vfs", "ignore"];
```

（`config.vue` 用前者，`modules.vue` 用后者，md3 与 miuix 各一份。）

状态页在 `magicCount` 之后增加：

```ts
const vfsCount = computed(() => sysStore.state?.mode_stats.vfs ?? 0);
```

并在 `webui/src/locales/*.json` 的 `magicModules` 相邻处为每种语言增加 `"vfsModules"` 文案（zh-CN：`"VFS 模块"`，en-US：`"VFS Modules"`，其余语言按同样语义翻译）。

- [ ] **步骤 4：运行测试、类型检查与构建**

运行：`cd webui && pnpm test -- --run`
预期：全部 PASS。
运行：`cd webui && pnpm typecheck`
预期：无类型错误。
运行：`cd webui && pnpm build`
预期：构建成功。

- [ ] **步骤 5：提交**

```bash
git add webui/src
git commit -m "feat(webui): expose vfs mode and status"
```

---

## 任务 14：文档同步与全量门禁

**文件：**
- 修改：`docs/ARCHITECTURE.md`（流水线、代码分层、CLI 契约）
- 修改：`docs/README_ZH.md` 与 `docs/README_EN.md`（模式说明）

**接口：**
- 依赖输入：任务 1–13 的全部产出。

- [ ] **步骤 1：更新架构文档**

在 `docs/ARCHITECTURE.md` 的「启动流水线」代码块中，把 `→ 再执行 Magic Mount` 之后补上：

```text
  → 再执行 VFS 注入（NoMount 兼容 Provider，二选一）
```

在「代码分层」的 `src/magic_mount/` 之后增加一条：

```markdown
- `src/vfs/`：NoMount 兼容的 VFS 后端。`rule.rs` 把共享树映射为规则，`protocol.rs`
  编解码 `nm_payload`，`sys.rs` 通过 keyring `add_key` 发送并维护页对齐缓冲，
  `backend.rs` 选择唯一活动的内核 Provider（设备已有的 NoMount 或 HM 自有实现，
  二选一、不并存、不热切换），`exec.rs` 应用规则并统计。
```

在「共享节点树契约」末尾补充 VFS 语义：

```markdown
VFS 目标不得存在被 Overlay/Magic 挂载的祖先目录（反之亦然），否则 plan 阶段报
`PlanConflict`。VFS 不是真实挂载，因此不进入 `active_mounts`，也不参与 KSU
try-umount 列表；其成功目标记录在 `vfs_active_mounts`。
```

在「CLI 契约」的 `status` 说明中补充 `vfs_modules`、`vfs_active_mounts`、`vfs_provider`。

- [ ] **步骤 2：更新多语言 README 模式说明**

在 `docs/README_ZH.md` 与 `docs/README_EN.md` 中把可选后端从「OverlayFS / Magic Mount」更新为「OverlayFS / Magic Mount / VFS（NoMount 兼容）」，并注明 VFS 需要兼容内核或独立 LKM。

- [ ] **步骤 3：运行禁用符号检查**

运行：`grep -rnE 'dbg!|\\.expect\\(|todo!|unimplemented!|\\.unwrap\\(\\)' src/vfs src/config.rs src/plan/mod.rs src/state.rs src/errors.rs || true`
预期：生产代码无命中（测试文件内的 `unwrap` 允许，需人工确认命中都在 `*_tests.rs` 或 `#[cfg(test)]` 内）。

- [ ] **步骤 4：运行全量门禁**

按 hm-verify 技能执行：

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
shellcheck module/*.sh tests/shell/*.sh
cargo check -p hybrid-mount --target aarch64-linux-android
cargo check -p hybrid-mount --target armv7-linux-androideabi
cargo check -p hybrid-mount --target x86_64-linux-android
cd webui && pnpm test -- --run && pnpm typecheck && pnpm build
```

预期：全部通过；任何一项失败都必须修复后重跑，不得跳过。

- [ ] **步骤 5：提交**

```bash
git add docs
git commit -m "docs: document the vfs backend"
```

---

## 自检结果

**1. 规格覆盖度**

| 规格章节 | 对应任务 |
| --- | --- |
| §6 wire 协议 | 任务 6（编解码）、任务 7（keyring 发送）、任务 8（Provider 调用） |
| §7.2 Provider 二选一 | 任务 8（`select_provider`）、任务 11（运行期绑定与回滚） |
| §7.3 状态与可观测性 | 任务 9、任务 13 |
| §8.1 `Mode::Vfs`、`vfs_strict` | 任务 1、任务 12 |
| §8.2 `MountPlan.vfs_module_ids` | 任务 2 |
| §8.3 `RunState` 字段与 `ModeStats.vfs` | 任务 9 |
| §9 规则映射 | 任务 4 |
| §10 遮蔽与冲突不变量 | 任务 3 |
| §11 执行顺序与回滚 | 任务 11 |
| §12 配置/CLI/WebUI 契约 | 任务 12、任务 13 |
| §15 测试与验证 | 每个任务的 TDD 步骤 + 任务 14 全量门禁 |
| §13 K2 内核实现、§7.1 内核集成 | 不在本计划，另立内核子系统计划 |
| §14 归属/co-author | 内核实现与集成提交时执行；用户态提交按需加 `Co-authored-by` |

**2. 占位符扫描：** 本计划无 `TODO`/`TBD`/“后续实现”类占位；唯一的外部依赖（K2 内核）已在范围说明中显式排除并指向独立计划。

**3. 类型一致性：** `build_vfs_rules`、`VfsRule`、`VfsAction`、`encode_rule`、`EncodedRule`、`build_add_rule_payloads`、`parse_version`、`ensure_status`、`VfsKernel`、`LkmLoader`、`select_provider`、`SUPPORTED_VERSIONS`、`apply_plan`、`VfsExecStats`、`KeyringKernel`、`PageBuffer`、`add_key` 在任务间的名称与签名一致；`RunState` 字段在任务 9 定义、任务 11 赋值、任务 13 消费，名称一致。

---

## 执行交接

计划已完成并保存至 `docs/superpowers/plans/2026-09-14-nomount-vfs-userspace.md`。两种执行方式：

**1. 子代理驱动（推荐）**——每个任务分发全新子代理，任务间进行评审，迭代更快。必需子技能：使用 superpower-subagent-driven-development。

**2. 内联执行**——在当前会话中使用 executing-plans 批量执行，设置检查点。必需子技能：使用 superpower-executing-plans。

请选择哪种方式。













