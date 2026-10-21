# 启动与系统应用可见性排查

本页记录 #500、#501 的代码核查和验证边界；本地测试通过不代表已在报告人的设备上复现或解决。

## APatch 与启动时生成的文件（#500）

2026-10-21 核对的 [APatch 启动流程](https://github.com/bmax121/APatch/blob/e8e4651c16fb5075882a25813c8732285fdb3505/apd/src/event.rs#L189)
先执行 metamodule 的 `metamount.sh`，再执行普通模块的 `post-fs-data.sh`，最后进入阻塞的
`post-mount` 阶段。其脚本环境包含 `APATCH=true`。

[MSD v2.5 的生成脚本](https://github.com/chenxiaolong/MSD/blob/v2.5/app/module/post-fs-data.sh)
从系统 `plat_seapp_contexts` 和各模块片段生成模块内的同名文件。Shell 会先截断输出文件，
再运行读取系统文件的命令。如果 Magic/VFS 已把这个模块源文件映射到系统路径，两者引用
同一文件，截断会同时影响输入，丢掉系统原有规则。Overlay 的复制层不共享这个源 inode。

Hybrid Mount 因此在 Rust 中选择挂载阶段：APatch 使用 `post-mount`，KernelSU 使用
`metamount`；两个脚本只转发阶段名。既有 Rust 进程锁和启动去重继续负责重复回调。
不修改模块源文件，不修改 SELinux 策略，也不关闭 SELinux。

回归测试用文件硬链接重现相同的截断别名效应，并验证挂载前生成文件仍包含系统规则。
报告人的 APatch 版本、完整启动日志及实机重启结果仍需确认。

## init 能看到 APK，应用看不到（#501）

不能仅凭 `/proc/<pid>/root` 的差异断定挂载发生得太晚。KernelSU 的
[应用配置](https://kernelsu.org/guide/app-profile.html#umount-modules) 支持按应用卸载模块挂载；
相关选项和 Zygisk/SUSFS 的处理也可能造成不同进程看到不同文件。

在 root shell 中，对实际不可见的 APK 收集以下只读信息；示例路径需替换为实际路径：

```sh
target=/system/app/com.termux/com.termux.apk
for name in init system_server zygote64 zygote com.termux; do
  for pid in $(pidof "$name"); do
    echo "=== $name pid=$pid ==="
    readlink "/proc/$pid/ns/mnt"
    ls -lZ "/proc/$pid/root$target"
    cat "/proc/$pid/mountinfo"
  done
done
logcat -d -s HybridMount
```

- 保留命令的完整错误，区分文件不存在和权限不足。
- 若仅应用进程缺失，先核对该应用的“卸载模块”配置，再重启应用对比。
- 若 zygote 本身就缺失，需要同时核对挂载时序、zygote 相关模块和实际 Root 版本。
- `lowerdir` 中的临时路径在清理后不可遍历，本身不能证明已挂载的 Overlay 内容丢失。
- 不应为绕过未知原因而强制向所有进程重放挂载，或全局关闭应用隔离。

当前规划回归覆盖普通应用目录的 30 个 APK、特权应用目录的 3 个 APK 和权限 XML，
逐一验证按模块切换 Overlay/Magic/VFS 后文件仍属于对应后端，其他模块仍使用原后端。
该测试验证规划，不能代替真实挂载或进程命名空间测试。

## 计数、后端选择与原生库

`mount_stats.files_mounted` 统计 Magic 的文件 bind，Overlay 使用 `overlayfs_mounts`，
Magic 的目录挂载另计 `magic_dirs`。Overlay 模式下前者为零不等于 APK 未准备好。
若 Magic 模式下 33 个 APK 的文件计数仍为零，应提供完整 `HybridMount` 日志和状态快照。

只切换一个模块可修改 `/data/adb/hybrid-mount/config.toml` 后重启：

```toml
[rules.custom_systemapps]
default_mode = "vfs"
```

先确认设备上的 VFS provider 可用；此配置不会使不兼容的内核自动获得 VFS 能力。

APK 能加载但报 `UnsatisfiedLinkError` 是另一个问题。Hybrid Mount 的三个后端均不承担
APK 安装或解包职责，不自动生成原生库目录。模块需提供设备 ABI 对应的系统应用布局，
例如 `system/app/<pkg>/lib/arm64/*.so`。具体路径取决于包布局及系统的 ABI 选择，参见
[Android 的路径计算](https://android.googlesource.com/platform/frameworks/base/+/master/services/core/java/com/android/server/pm/PackageAbiHelperImpl.java)。
不能用调整挂载传播代替缺失库文件，也不能把 VFS 规划器当作 APK 安装器。
