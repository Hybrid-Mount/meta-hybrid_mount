# Hybrid Mount WebUI

<img src="https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/main/icon.svg" align="right" width="120" />

![语言](https://img.shields.io/badge/Language-TypeScript-3178C6?style=flat-square&logo=typescript)
![框架](https://img.shields.io/badge/Framework-SolidJS-2C4F7C?style=flat-square&logo=solid)
![平台](https://img.shields.io/badge/Platform-Android%20%2F%20KernelSU-green?style=flat-square&logo=android)
![许可证](https://img.shields.io/badge/License-Apache--2.0-blue?style=flat-square)

Hybrid Mount WebUI 是 **Hybrid Mount** 的 Material Design 3 前端控制台，
面向 KernelSU 场景提供配置管理、模块策略编辑、运行诊断和运维动作入口。

**[English](README.md)** &nbsp; **[简体中文](README.zh-CN.md)**

---

## 目录

- [设计目标](#设计目标)
- [架构说明](#架构说明)
- [国际化](#国际化)
- [仓库结构](#仓库结构)
- [接口契约](#接口契约)
- [构建与开发](#构建与开发)
- [适配说明](#适配说明)
- [许可证](#许可证)

---

## 设计目标

1. **运行可观测**：清晰展示挂载策略与模块策略状态。
2. **接口稳定**：通过统一 `AppAPI` 维持前后端边界。
3. **默认安全**：真实环境与 Mock 环境切换具备可控回退。
4. **便于适配**：支持替换后端实现而不改动主要页面逻辑。

## 架构说明

WebUI 运行流程如下：

1. 读取配置与运行时元信息。
2. 扫描模块清单并渲染策略控制。
3. 保存全局配置或模块级规则。
4. 查询系统/设备状态及 daemon 日志。
5. 在需要时触发运维动作（如重启）。

核心分层：

- `src/routes`：状态/配置/模块/信息等标签页。
- `src/lib/api.ts`：统一后端桥接层（`AppAPI`、`RealAPI`、`MockAPI`）。
- `src/lib/types.ts`：配置、模块、状态等数据契约。
- `src/lib/stores/*`：配置/系统/模块/UI 状态管理。

## 国际化

WebUI 目前提供以下语言：

- 英语 (`en-US`，默认)
- 西班牙语 (`es-ES`)
- 意大利语 (`it-IT`)
- 日语 (`ja-JP`)
- 俄语 (`ru-RU`)
- 乌克兰语 (`uk-UA`)
- 越南语 (`vi-VN`)
- 简体中文 (`zh-CN`)
- 繁體中文 (`zh-TW`)

README 文档提供 [English](README.md) 和 [简体中文](README.zh-CN.md) 版本。

## 仓库结构

```text
.
├─ src/
│  ├─ routes/           # 标签页与页面视图
│  ├─ components/       # 可复用 UI 组件
│  ├─ lib/              # API 桥接、状态管理、常量与类型
│  └─ locales/          # 国际化词典
├─ public/              # 静态资源
├─ package.json         # Node 脚本与依赖
└─ README*.md           # 项目文档
```

## 接口契约

前端依赖统一抽象 `AppAPI`（定义于 `src/lib/api.ts`）。

| 方法              | 入参                | 返回                     | 说明                |
| ----------------- | ------------------- | ------------------------ | ------------------- |
| `loadConfig`      | -                   | `Promise<AppConfig>`     | 读取当前全局配置。  |
| `saveConfig`      | `AppConfig`         | `Promise<void>`          | 保存全局配置。      |
| `resetConfig`     | -                   | `Promise<void>`          | 重置/重新生成配置。 |
| `scanModules`     | `path?: string`     | `Promise<Module[]>`      | 扫描模块与规则。    |
| `saveModules`     | `Module[]`          | `Promise<void>`          | 批量保存模块规则。  |
| `saveModuleRules` | `moduleId`, `rules` | `Promise<void>`          | 保存单模块规则。    |
| `getStorageUsage` | -                   | `Promise<StorageStatus>` | 查询存储后端模式。  |
| `getSystemInfo`   | -                   | `Promise<SystemInfo>`    | 查询系统运行诊断。  |
| `getDeviceStatus` | -                   | `Promise<DeviceInfo>`    | 查询设备信息。      |
| `getVersion`      | -                   | `Promise<string>`        | 获取模块/应用版本。 |
| `openLink`        | `url: string`       | `Promise<void>`          | 打开外部链接。      |
| `reboot`          | -                   | `Promise<void>`          | 执行重启动作。      |
| `readLogs`        | -                   | `Promise<string>`        | 读取 daemon 日志。  |

核心数据结构（`src/lib/types.ts`）：

- `AppConfig`：`moduledir`、`mountsource`、`partitions`、`overlay_mode` 及功能开关。
- `ModuleRules`：模块默认模式与路径级覆盖策略。
- `Module`：模块元数据、挂载状态与策略状态。
- `SystemInfo` / `DeviceInfo`：运行可观测信息。

## 构建与开发

环境要求：

- Node.js 20+
- pnpm 9+

常用命令：

```bash
pnpm install
pnpm dev
pnpm build
pnpm preview
pnpm lint
```

`pnpm dev` 默认使用 `MockAPI`。如需在开发服务器中连接真实 KernelSU bridge，可使用 `VITE_USE_MOCK=false pnpm dev`。

## 适配说明

- 错误返回建议使用 **结构化语义**（错误码 + 错误消息），避免 UI 解析原始 stderr。
- 扩展 `AppConfig`、`Module`、`SystemInfo` 时保持 **向后兼容**。
- 所有 shell 入参需进行 **转义或等效安全处理**。
- `save*` 类接口建议保持 **幂等性**，便于脚本编排与恢复。
- 保持状态文件与日志路径可读，提升排障效率。

## 许可证

本项目采用 [Apache-2.0](LICENSE)。
