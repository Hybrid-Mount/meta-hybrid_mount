# VFS 生命周期上游对照

基线：d39c5925。2026-11-05 在 Ubuntu 26.10 / WSL2（6.18.58.1-microsoft-standard-WSL2）检查。

## 决定

保留现有 superblock 活跃引用和回调驻留保护，不直接移植 NoMount bd5e2d11f6e8 的回退，也不引入 04af0552bb29 的 workqueue 排空实现。
本实现没有相同的 reclaim workqueue；RCU/SRCU 与退出路径不是该补丁的直接适用对象。
这不是“卸载/关机问题已修复”的结论：内建模式延迟文件系统关闭的风险仍存在，需要具有本模块的可销毁内核环境验证。

## 引用配对

| 对象 | 获取 | 释放 / 保留 |
| --- | --- | --- |
| 原生父路径 | 成功发布规则/生成祖先时取得 parent_path | 规则退役经读者排空后 hm_free_rule 释放 |
| backing path | kern_path、resolver owned snapshot | 规则、快照及代理 inode 各自释放所拥有引用 |
| superblock | hijack_superblock 成功发布前 s_active +1，调用者仍有路径引用 | restore_superblocks 恢复原操作表后 deactivate_super |
| 本模块 | 发布 superblock 回调前 try_module_get | 故意保留至重启；规则 clear 不开放热卸载 |
| 原生 filesystem module | 目录 fops hook 时获取 owner | 目录 fops allocation 保留，exit 排空 RCU 后释放 owner |
| 目录拓扑 | 发布所有权及 sleeping reader refs | 规则/eviction 退役与 sleeping reader 各自释放 |

clear rules 不等于 restore superblocks。当前驻留保护避免已保存的回调表指向被卸载代码；不能仅为释放 s_active 而提前释放表。

## 已运行的证据

- `python3 tests/kernel/hook_transaction.py`：18 个场景通过。已涵盖各安装分配/模块获取失败、重复 hook 不重复取引用、恢复原表后才释放 superblock 活跃引用、export 回调驻留。
- `python3 tests/kernel/inode_lifecycle.py`：现有全部场景通过，覆盖真实父路径、退役/eviction 交错、缓存代际与 sleeping 所有权基础。
- `vfs_semantics.py`、`uid_cursor.py`、`reader_lifetime.py`、`child_rules.py`：现有场景均通过。
- 使用系统 cc 编译运行；环境未安装 clang，未将其报告为 clang 编译通过。

上述是提取生产函数的受控主机测试，不证明真实 VFS 在 MNT_DETACH/MNT_FORCE 或关机路径的行为。

## 尚未验证与后续门槛

当前 WSL 内核未集成本模块，不能复现本模块下的内建关机、真实设备 superblock 释放、已有 FD 与真实卸载交错。
正常卸载、lazy detach、支持强制卸载的文件系统、内建关机四类验证均记为尚未验证，不在此环境强行装入不匹配模块。
若未来证据支持改变引用策略，须明确同时处理回调表/模块代码/原生 filesystem 的完整生命周期并新增回归，再替换当前保护；单独删除 s_active 不满足该条件。
