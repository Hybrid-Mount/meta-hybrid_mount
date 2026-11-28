---
name: hm-webui
description: Hybrid Mount WebUI（Vue 3 + TS + Vite）开发与后端通信：pnpm 脚本、kernelsu.exec JSON 命令协议、持久化文件与构建注入。
whenToUse: 修改 webui/、对接 Rust 二进制命令、调试 WebUI 与控制端通信，或构建 WebUI 产物时。
---

# WebUI 开发

`webui/` = Vue 3 + TypeScript + Vite，包管理器 pnpm 11.22，Node >= 24（见 `webui/package.json`）。

## 脚本

```bash
cd webui
pnpm install --frozen-lockfile
pnpm dev        # vite --host，默认 http://localhost:5173
pnpm build      # 构建产物；打包时由 xtask 负责 MODULE_ID 注入
pnpm lint       # eslint . && prettier --check .
pnpm lint:fix
pnpm test       # vitest run && vue-tsc -b
pnpm typecheck  # vue-tsc -b
```

## 与 Rust 后端通信

WebUI 通过管理器提供的 `kernelsu.exec`（`kernelsu` npm 包）调用**同一个** Rust 二进制，
以 JSON 交换数据。命令见 `src/cli/mod.rs`：

| 命令 | 用途 |
| --- | --- |
| show-config | 读取当前配置 |
| save-config | 保存配置（**写入后需重启生效**） |
| gen-config | 生成配置模板 |
| modules | 模块列表快照 |
| status | 运行状态 |
| version | 版本 |
| install-state | 安装状态 |
| clear-mount-errors | 清除挂载错误 |
| emulated-soft-reboot | 模拟软重启（仅 linux/android） |

持久化文件：

- `/data/adb/hybrid-mount/config.toml`
- `/data/adb/hybrid-mount/run/state.json`
- `/data/adb/hybrid-mount/scan.ret`

约定：CLI 的 JSON 只走 stdout，诊断日志走 log/stderr，前端解析不要混入日志行。

## 构建产物与注入

- `cargo xtask build` 默认构建普通/Wear 双包，注入 `MODULE_ID` 与 `WEBUI_FLAVOR`，前端分别输出到 `output/webui-normal/`、`output/webui-wear/`。Wear 入口固定手表布局，普通入口不再探测设备类型。
- `output/` 是生成物，不要手改或提交。直接预览 Wear：`WEBUI_FLAVOR=wear pnpm dev`（在 webui 目录执行）。

## 修改 WebUI 的推荐流程

1. 改 `webui/src/`；
2. `pnpm lint && pnpm test`；
3. 需要真实交互时 `pnpm dev`，设备侧数据仍来自真机 kernelsu.exec；
4. 最终用 hm-verify 的 WebUI 段验证 lint / test / build。
