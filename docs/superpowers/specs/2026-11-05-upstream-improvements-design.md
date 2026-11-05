# 上游选择性吸收设计

日期：2026-11-05。状态：用户授权加入计划；实现前复核本设计和各计划，不自动同步上游分支。

## 目标与范围

将本次上游检查中有价值的改动转化为独立可验证任务，同时保留 Hybrid Mount 的 hm1 协议、ART/seqcount/SRCU、所有权账本、事务与三后端规则。
本次是选择性吸收，不升级 NoMount 内核协议，不引入 Lua 钩子，不将他人的回退视为本仓库已验证修复。
主机测试、内核编译和 Android 真机验证分别报告；此前 xattr 读取 CI 失败单独处理，不能归因为这里的 xattr_handler 兼容问题。

## 已考虑的做法

- 整体同步上游：分支实现与协议已分化，且存在实验提交与回退，不采用。
- 只收集提交标题：不能判断我们是否已有等效行为，不足以成为实施依据。
- 按行为验证后选择性移植：用户要求纳入计划，采用该方向；验证无缺口的任务以对照结论结束，不制造无意义代码差异。

## 纳入清单

| 项目 / 提交 | 本仓库处理 |
| --- | --- |
| Mountify [8ba7163f7a49](https://github.com/backslashxx/mountify/commit/8ba7163f7a49a70c25ab96193fe8ccd1f21773a7) | 子挂载边界保护并入分区映射计划 |
| meta-magic_mount-rs [f9cfa8b598e8](https://github.com/Tools-cx-app/meta-magic_mount-rs/commit/f9cfa8b598e8f38eaa63c75cc2bee9b4d03be26f) | .remove 标记兼容，独立交付 |
| NoMount [f0478cad4081](https://github.com/maxsteeel/nomount/commit/f0478cad4081) | 探测实际 xattr_handler->__get 成员，替代版本区间判断 |
| NoMount [1ecef88d598d](https://github.com/maxsteeel/nomount/commit/1ecef88d598d)、[e07896de51a3](https://github.com/maxsteeel/nomount/commit/e07896de51a3) | 对照隔离 UID 与已持有目录 FD/CWD 的行为；已等效分支只补必要验证 |
| NoMount [bd5e2d11f6e8](https://github.com/maxsteeel/nomount/commit/bd5e2d11f6e8)、[04af0552bb29](https://github.com/maxsteeel/nomount/commit/04af0552bb29) | 审查 superblock 引用、清理顺序与关机路径；不能直接删引用，也不凭空引入 workqueue |
| NoMount [66978f9c8d7a](https://github.com/maxsteeel/nomount/commit/66978f9c8d7a)、[940efd11b225](https://github.com/maxsteeel/nomount/commit/940efd11b225)、[0fb89eac1a5b](https://github.com/maxsteeel/nomount/commit/0fb89eac1a5b)、[ab8c05775122](https://github.com/maxsteeel/nomount/commit/ab8c05775122) | 第二阶段研究规则源文件 rename/原子替换；连同后续修正一起评估，不孤立移植早期版本 |

NoMount master 核对至 bf1847f98fed，dev 至 bd5e2d11f6e8；master 和 dev 的支持状态必须区分。
已有 opaque 支持、NoMount 20/21/22 通信支持不重复实施；不将范围 UID 管理自动移植到 hm1。
meta-magic_mount-rs 的 Lua 改造 6ffa1a1c91a4 暂缓，不属于本轮交付。

## VFS 处理边界

生命周期任务先形成证据和处置结论，覆盖内建和 LKM、清空规则后、已有打开文件、正常/延迟/强制卸载及关机。
强制卸载仅在可销毁的 Linux 测试环境执行，不修改用户设备；不支持强制卸载的文件系统明确记为不适用。
不能在缺少替代生命周期证明时删除 s_active 或模块驻留引用。若需改变引用机制，先将具体替代方案补入设计再实施；验证任务完成不等于该风险已修复。

UID 隔离的最低要求是：名单内 UID 不应因代理目录产生额外 ENOENT、错误隐藏或规则暴露。原生缓存仍走真实文件系统回调；未隔离 UID 的注入和 opaque 语义不变。
纯合成且没有原生对应对象的目录不存在可回退目标，应保持不可见。已有打开 FD 的权限语义单独验证，不承诺通过加隔离名单撤销已经取得的所有访问权限。

__get 特性探测必须同时覆盖 out-of-tree Makefile 和 setup.sh 生成的 in-tree Kbuild；同版本不同头文件都能选择正确分支。保留源码来源和预编译二进制来源校验。

源文件身份研究必须分别记录：源文件 rename、unlink/recreate、原子替换、经过符号链接的规则，以及 lookup/open/getattr 并发。先确定当前语义与上游差异；若改变用户可见的跟随/固定语义，补设计决定后再写产品代码。

## .remove 语义建议

- 只认目录中的精确文件名 .remove；ordinary.remove 等普通文件名不受影响。只接受普通标记文件，符号链接/目录伪装的标记报告无效，不跟随模块外路径。
- 优先于同目录 .replace；不遍历/挂载该目录的模块内容，标记本身永不挂载。模块根 remove（无点号）的原义不变。
- 隐藏目标目录本身及其子树，而非留下一个空目录；原磁盘内容不删除。以规范化 whiteout 进入共享树，再由现有后端机制执行，不改变 wire ABI。
- 路径规则仍匹配原模块路径；该目录的最终模式为 ignore 时不执行隐藏；父目录 .remove 生效后子路径规则不能重新挂入后代。
- 与其他模块对子树的贡献发生冲突时显式报告，不按扫描顺序吞掉内容；沿用现有后端冲突错误的事务语义，不擅自改为本设计之外的新降级策略。
- 禁止对逻辑 system/受管分区/extra_mount 根执行 .remove；报告并跳过该删除请求，保留无关分支。不可用分区也不执行 .remove。
- 有子挂载的目录仅在现有事务能安全隐藏并恢复相应可见树时执行，否则跳过该请求并给出诊断，不拆分成意义不同的“删除部分子项”。

## 验收与交付

三个计划按独立提交验证，不声称全部 NoMount 改动已吸收。内核源代码变化须重建受支持预编译模块并更新 sources.txt/list.txt 后才能打包交付。
本轮文档更新不执行测试设备操作、构建、提交代码或发布。
