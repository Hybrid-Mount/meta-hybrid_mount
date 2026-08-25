# Hybrid Mount Changelog

## v6.0.0 (unreleased)

### Features

- 重构 Hybrid Mount：使用单一 Rust 二进制与 `kernelsu.exec` 直调 CLI，不再依赖 daemon / HTTP / SSE
- 混合挂载 planner：路径规则 > 模块 default mode > 全局 default mode，冲突显式报错
- Magic Mount：Node 树、`.replace`、whiteout、tmpfs skeleton、只读 remount 与 KSU try-umount
- OverlayFS：fsopen + 传统 mount fallback、超过 64 层时分段、mountinfo 子挂载重建与文件级 shallow layer
- storage 后端：tmpfs 与系统 mke2fs 格式化的动态大小 ext4 loop 镜像，挂载后接入 KernelSU ext4 sysfs nuke
- 运行临时目录改用无项目特征的随机名称，并按 `/tmp` → `/tmp/rw` → `/mnt` 回退
- 使用 `scan.ret` 与 `run/state.json` 连接启动流水线和 WebUI，不引入常驻 daemon
- Vue 3 双 UI：MD3（默认）+ Miuix，共享 lib 层与 11 个 locale
- module 安装脚本：symlink-only 分区处理与模块源目录只读约束
- xtask 构建打包、TG 通知及 CI/Release 连线
