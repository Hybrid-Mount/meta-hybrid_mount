# ReHybrid-Mount Changelog

## v6.0.0 (unreleased)

### Features

- 从零重建 ReHybrid-Mount:唯一 Rust 二进制 + `kernelsu.exec` 直调 CLI,无 daemon / HTTP / SSE
- 混合挂载 planner:路径规则 > 模块 default_mode > 全局 default_mode,冲突显式报错
- Magic Mount(Node 树、`.replace`、whiteout、tmpfs skeleton、只读 remount、KSU try-umount)
- OverlayFS(fsopen + fallback、>64 层 staging、mountinfo 子挂载重建)
- storage 后端:tmpfs 与 ext4 loop 镜像
- Vue 3 双 UI:Miuix(默认)+ MD3,共享 lib 层,11 locale
- module 安装脚本:symlink-only 分区处理,模块目录铁律
- xtask 构建打包、TG 通知、CI/Release 继承现有连线
