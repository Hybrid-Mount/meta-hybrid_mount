---
name: hm-rust-android
description: Hybrid Mount Rust Android 交叉编译与平台条件检查；用于修改 sys、storage、挂载后端或排查 macOS 通过而 Android 失败的问题。
---

# Rust Android 平台检查

先读 Cargo.toml、rust-toolchain.toml、.cargo/config.toml 和 .github/workflows/lints.yml，以当前文件为准。项目使用 Rust 2024、nightly；Android 构建由 xtask 管理，最低 API 26。

- macOS 不编译 cfg(linux/android) 下的 rustix、loopdev、ksu 调用；宿主机测试通过不代表 Android 分支通过。
- 修改平台代码后检查相关目标；全量验证覆盖 aarch64-linux-android、armv7-linux-androideabi、x86_64-linux-android。缺目标时用 rustup target add <target> --toolchain nightly，再运行 cargo check -p hybrid-mount --target <target>。
- 需要链接或打包时先读 xtask/src/main.rs 的 NDK 配置方式，使用 cargo xtask build。不要凭空设置 linker 或修改项目工具链。
- 规则解析与规划回归优先写不需要 root 的测试；系统挂载、loop、ioctl 和 LKM 行为需要相应 Android 环境。不要在开发宿主机运行启动挂载流水线。
- 检查 32 位 armv7 的整数宽度、文件描述符所有权、CString/NUL、平台 cfg，以及错误路径上的资源释放。
- CI 的 Rust 基础门禁为 cargo fmt --all -- --check、cargo clippy --workspace --all-targets -- -D warnings、cargo test --workspace。按改动范围执行，完整 CI 使用 hm-verify。

汇报实际运行的目标与结果，分清编译检查、宿主机测试和真机运行；缺少环境时明确尚未验证的部分。
