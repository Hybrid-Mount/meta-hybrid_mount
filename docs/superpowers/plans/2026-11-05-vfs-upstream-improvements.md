# VFS Upstream Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 验证 VFS 生命周期与 UID 隔离边界，选择性引入已证明适用的上游改进。

**Architecture:** 保持 hm1 与既有所有权/RCU/SRCU 实现。先验证高风险边界，再移植独立的头文件探测；源文件身份研究作为单独后续任务。

**Tech Stack:** C、Kbuild、Python 生产函数抽取测试、Shell、DDK、Linux 隔离挂载命名空间。

**Spec:** [上游选择性吸收设计](../specs/2026-11-05-upstream-improvements-design.md)

## Global Constraints

- 不盲删 s_active/模块驻留引用，不自动同步 NoMount dev，不改变 hm1 wire ABI。
- 内建与 LKM 编译路径均验证；设备测试仅在另行安排的测试环境执行。
- 先复现缺口再修改；发现等效行为以验证结论结束，不能为了“移植”制造重复实现。
- 每项源修改独立提交；内核二进制发布前必须完成重建和来源一致性校验。

## Review Focus

- 清空规则后仍有 FD 和操作表引用：K1 覆盖，不能提前释放回调。
- 原生与合成目录持有 FD 后改变 UID 隔离：K2 分别验证，不混同权限撤销与路径查找。
- 同版本厂商头文件成员不同：K3 同时使用带/不带 __get 的夹具。
- setup.sh 生成 Kbuild 与外置 Makefile 不一致：K3 验证两种接入。
- rename 与原子替换并发：K4 分别记录已有 FD 与新 lookup 的结果。

### Task 1: K1 — 生命周期处置报告（优先，不预设删除引用）

**Files:** Read `module/vfs/src/hybridmount.c`, `module/vfs/src/hybridmount.h`, `module/vfs/src/PROVENANCE`; Test `tests/kernel/hook_transaction.c`, `tests/kernel/hook_transaction.py`, `tests/kernel/inode_lifecycle.c`, `tests/kernel/inode_lifecycle.py`; Create `docs/reviews/2026-11-05-vfs-upstream-lifetime.md`。

**Interfaces:** 消费现有 hook/restore/clear/exit 路径；产出上述报告，逐项标记“已等效 / 已复现缺口 / 环境不适用 / 尚未验证”，列出保持不变或后续替代方案。

- [ ] 画出 s_active、module ref、path ref、打开文件及操作表的取得/释放配对，核对 bd5e2d11f6e8 与 04af0552bb29 在本实现中的适用性。
- [ ] 将可模拟的反例加入生产函数抽取 harness：清空规则不提前释放仍可达回调；安装失败引用平衡；恢复后不重复释放。运行 `CC=clang python3 tests/kernel/hook_transaction.py`、`CC=clang python3 tests/kernel/inode_lifecycle.py`，缺口必须先观察到具体 FAIL；原已正确场景直接保留 PASS。
- [ ] 在可销毁 Linux 环境验证正常/延迟/适用的强制卸载、内建关机和打开 FD 场景，记录内核/文件系统/日志。没有环境时报告 blocked validation，不把主机 harness 当作通过。
- [ ] 报告给出证据支持的最小处理决定。若需新生命周期方案，先补设计再实施，不在本步骤盲目删除引用或引入上游 workqueue。
- [ ] 只提交经审查的报告与相关回归用例；只有具体修复验证通过才以 fix 描述提交。

### Task 2: K2 — UID 隔离的持有目录边界

**Files:** Modify `module/vfs/src/hybridmount.c`（仅复现后）, `tests/kernel/inode_lifecycle.c`, `tests/kernel/inode_lifecycle.py`, `tests/kernel/vfs_semantics.c`, `tests/kernel/vfs_semantics.py`; Create `tests/device/vfs_uid_directory_view.sh`。

**Interfaces:** 保留 `hm_dir_lookup(struct inode *, struct dentry *, unsigned int)`、`hm_d_revalidate_common(...)` 签名；消费 `hybridmount_is_uid_blocked`，不新增 ioctl/协议命令。设备用例只添加并回收自己拥有的规则/UID，不全局 clear。

- [ ] 增加 `blocked-held-native-dir`、`blocked-held-synthetic-dir`、`unblocked-opaque-dir`、`blocked-native-revalidate` 场景，断言有原生对应的子项走原生视图，纯合成对象不可见，未隔离视图不变，原生回调调用次数正确。
- [ ] 运行 `CC=clang python3 tests/kernel/inode_lifecycle.py` 和 `CC=clang python3 tests/kernel/vfs_semantics.py`；分别记录哪些现有场景已 PASS，哪些反例 FAIL，不能先假定上游问题在本仓库存在。
- [ ] 对已复现分支实现最小修正，保持原生文件系统 lookup/权限检查，不给伪造 dentry 调用未经初始化的原生回调。
- [ ] 重跑上述 harness，设备脚本另验证先持有目录、再隔离、再 lookup/openat/readdir 的顺序；无设备时保留未验证状态。
- [ ] 更新来源说明并独立提交；源码变更进入 K5 的重建清单。

### Task 3: K3 — xattr __get 特性探测

**Files:** Modify `module/vfs/src/Makefile`, `module/vfs/setup.sh`, `module/vfs/src/hybridmount.c`, `module/vfs/src/PROVENANCE`, `tests/shell/vfs_setup.sh`; Create `tests/shell/vfs_xattr_feature.sh`; Modify `.github/workflows/lints.yml`。

**Interfaces:** Kbuild 产生 `HM_HAVE_XATTR__GET` 宏；`hm_xattr__get` 定义和代理安装使用同一宏。setup.sh 生成相同探测，无配置/ABI变化。

- [ ] 用同版本的两个最小 xattr.h 夹具分别包含/不包含 __get 函数指针；加入空白、换行声明，以及 setup.sh 生成 Kbuild 一致性断言。
- [ ] 执行 `sh tests/shell/vfs_xattr_feature.sh`，确认旧版本判断不能通过所需分支选择测试。
- [ ] 在外置及内建 Kbuild 加入成员特性探测，替换 C 中两处版本区间判断。若头文件文本扫描不能正确处理真实声明，使用小型编译探测，不扩展至无关内核 API。
- [ ] 执行上述测试、`sh tests/shell/vfs_setup.sh`、现有 C harness；接入 lint workflow，核对支持的旧内核和 GKI 编译。
- [ ] 更新 PROVENANCE，独立提交，通过 K5 交付源码和二进制一致的包。

### Task 4: K4 — 源文件身份与重命名研究（后续，不预设新语义）

**Files:** Read `module/vfs/src/hybridmount.c`, `module/vfs/src/hybridmount.h`; Create `docs/reviews/2026-11-05-vfs-source-identity.md`, `tests/device/vfs_source_identity.sh`; Extend `tests/kernel/inode_lifecycle.c` / `.py`（有明确可模拟断言时）。

**Interfaces:** 产出对照报告：每种操作分别记录旧 FD 与新 lookup/open/getattr 的对象身份和错误码；不在研究阶段新增 rebind API。

- [ ] 分析 66978f9c8d7a → 940efd11b225 → 0fb89eac1a5b → ab8c05775122 的依赖与后续修正，区分 inode 绑定和目录项名称快照。
- [ ] 准备 rename、unlink/recreate、atomic replace、符号链接来源及并发场景；固定初始/替换内容为 `old` / `new`，报告实际读到哪个及 inode 身份，不用一个布尔成功掩盖语义差异。
- [ ] 运行对照实验，检查缓存代际、原生权限和 fd/getattr 一致性。未运行设备用例必须注明。
- [ ] 若需改变跟随/固定语义，先形成具体设计选择供确认，再添加正式实现任务；否则记录不移植的依据。
- [ ] 独立提交研究报告和可复用验证脚本，不以“已修复”描述仅完成研究的成果。

### Task 5: K5 — 内核交付门槛（每次内核源码交付均执行）

**Files:** `module/vfs/binaries/`, `module/vfs/src/PROVENANCE`, `module/vfs/README.md`, `.github/workflows/` 下现有 VFS 构建流程。

**Interfaces:** 消费已验证的具体源码修订；产出对应二进制、`sources.txt`、`list.txt` 和内建/LKM构建记录，不改导入基线 digest 冒充最新来源。

- [ ] 跑现有全部 `tests/kernel/*.py` 的正确性 harness（benchmark 非强制门槛），并运行 `sh tests/shell/vfs_setup.sh`。
- [ ] 按现有支持矩阵重建；单目标命令示例 `ddk build --target android14-6.1 -- -C module/vfs/src`，同时覆盖旧内核与内建接入，不用单目标结果宣称全矩阵通过。
- [ ] 更新二进制来源记录，运行 `sh tests/shell/vfs_sources_digest.sh`、`sh tests/shell/vfs_sources_digest_test.sh` 和 SHA-256 清单校验，期望全部退出 0。
- [ ] 报告“主机/编译/设备”三类结果；源码和预编译来源不一致时不打包发布。提交交付资料，不自动发布或刷机。
