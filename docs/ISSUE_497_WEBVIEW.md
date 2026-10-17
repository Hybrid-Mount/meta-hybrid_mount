# #497 WebView 挂载隐藏排查

核查日期：2026-10-17。结论限于源码和 issue 提供的日志，尚无对应设备复测。

## 结论与范围

[报告 #497](https://github.com/Hybrid-Mount/meta-hybrid_mount/issues/497)
描述了模块 RRO 路径在主 zygote 可见、在 UID 1053 的 webview_zygote 不可见，
以及按原路径重新打开继承的 APK 文件描述符失败。该证据支持挂载可见性与
WebView fork 失败有关，但不能单凭 userspace 版本号确定设备实际使用的内核策略。

本次不修改 KernelSU、不添加 RRO 全局白名单、不关闭 Hybrid Mount 全局卸载。
精确豁免应先核实设备已有的 WebView 专用控制；本项目没有独立的按进程豁免接口。

## 上游已有实现

- KernelSU [#3630](https://github.com/tiann/KernelSU/pull/3630)，
  2026-08-25 合入 `4521784328352c54334beb29e05c74360b60d7cb`，
  调整 WebView 卸载策略，并提供 `webview_zygote_umount` 专用开关。
  上游说明这项策略也用于防止其他 isolated 进程读取 WebView 子进程的挂载信息；
  因此“关闭开关”不能直接等同于“渲染进程仍然隐藏”。
- KernelSU [#3666](https://github.com/tiann/KernelSU/pull/3666)，
  2026-08-30 合入 `3497a56261564ee7fca494e3bfa0fac80118a34d`，
  移除旧的 feature 开关，改为应用配置中的特殊条目 **WebView Zygote**：
  profile key 为 `webview_zygote`、UID 为 1053，独立控制 `umount_modules`。
  这不是按 WebView APK 包名配置，也不需要授予 root。
- KernelSU Next [#1477](https://github.com/KernelSU-Next/KernelSU-Next/pull/1477)
  于 2026-08-30 合入同一配置方案。

各分支、管理器与内核版本可能不同。旧版本的专用 feature 与新版本的应用配置
不能混用，也不能仅从管理器显示了条目就断定内核行为一致。

## 本项目调用边界

- `src/overlayfs/overlayfs.rs` 构建 overlay 后调用 `send_unmountable`；
  Magic Mount 和 pipeline 也通过相同注册入口。
- `src/utils/ksu.rs::send_unmountable` 只排队路径；
  `commit_unmount_list` 才提交给驱动。
- `src/utils/ksu_umount.rs::UmountCommand` 只有路径指针、flags、mode，
  没有 UID、进程名或 SELinux 域字段。
- `src/utils/ksu_driver.rs::manage_umount` 发送 `MANAGE_TRY_UMOUNT`，
  不选择目标进程，也不修改其应用配置。
- `src/runtime/device.rs::enter_init_namespace` 是运行时管理操作进入 init
  挂载命名空间的入口，不是进入 WebView 或应用进程命名空间。

在已检查的路径中没有发现绕开 KernelSU 进程策略、单独对 WebView 执行卸载的代码。
撤回 `/product/overlay` 等路径会同时改变其他应用的卸载范围，不符合本次要求。

## 设备验证要求

需要报告者提供实际内核端 KernelSU 分支、版本或 commit，管理器版本，以及
WebView 专用配置的当前状态。不要把另一份 VFS insmod bugreport 的内核信息套用到此 issue。

若内核与管理器已支持专用配置，可先仅对 **WebView Zygote / UID 1053**
关闭“卸载模块”，保持普通应用的卸载设置不变，并重启后比较：

1. WebView server 的 RRO 路径重新可见，且重复创建 WebView 不再出现相同 ENOENT。
2. 普通应用仍看不到模块挂载。
3. WebView sandboxed renderer 仍看不到模块挂载；与 server 比较其挂载命名空间
   和 mountinfo，不能只检查 server 恢复或界面能打开。

这只是已有配置的验证步骤，不是已验证修复。如果 renderer 继承 server 的可见挂载，
即使 WebView 恢复也不满足“仅 server 精确豁免”的要求。若专用配置已关闭却仍复现，
应继续定位实际执行卸载的驱动或注入组件，而不是扩大本模块的路径豁免范围。

本次仅新增排查文档；没有修改运行时代码、KernelSU 配置或其他仓库，也没有发布 issue 回复。
