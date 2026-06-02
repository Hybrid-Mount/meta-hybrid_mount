# Hybrid Mount WebUI

<img src="https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/main/icon.svg" align="right" width="120" />

![Language](https://img.shields.io/badge/Language-TypeScript-3178C6?style=flat-square&logo=typescript)
![Framework](https://img.shields.io/badge/Framework-SolidJS-2C4F7C?style=flat-square&logo=solid)
![Platform](https://img.shields.io/badge/Platform-Android%20%2F%20KernelSU-green?style=flat-square&logo=android)
![License](https://img.shields.io/badge/License-Apache--2.0-blue?style=flat-square)

Hybrid Mount WebUI is a Material Design 3 frontend for **Hybrid Mount** runtime management.
It provides a stable UI contract for configuration, module policy editing, runtime diagnostics, and operational actions in KernelSU environments.

**[English](README.md)** &nbsp; **[简体中文](README.zh-CN.md)**

---

## Table of Contents

- [Design Goals](#design-goals)
- [Architecture](#architecture)
- [Internationalization](#internationalization)
- [Repository Layout](#repository-layout)
- [API Contract](#api-contract)
- [Build and Development](#build-and-development)
- [Adapter Notes](#adapter-notes)
- [License](#license)

---

## Design Goals

1. **Operational clarity** for mount strategy and module policy status.
2. **Stable integration boundary** via a single `AppAPI` interface.
3. **Safe defaults** with controlled fallbacks between real and mock environments.
4. **Adaptation-friendly structure** for alternative backend implementations.

## Architecture

At runtime, the WebUI follows this flow:

1. Load configuration and runtime metadata.
2. Scan module inventory and render policy controls.
3. Persist global config or module-level rules.
4. Query system/device status and daemon logs.
5. Trigger operational actions (such as reboot) when required.

Core frontend layers:

- `src/routes`: page-level tabs for status/config/modules/info.
- `src/lib/api.ts`: unified backend bridge (`AppAPI`, `RealAPI`, `MockAPI`).
- `src/lib/types.ts`: shared data contracts for config, module, and status payloads.
- `src/lib/stores/*`: state containers for config/system/module/UI domains.

## Internationalization

The UI currently ships with these locales:

- English (`en-US`, default)
- Español (`es-ES`)
- Italiano (`it-IT`)
- 日本語 (`ja-JP`)
- Русский (`ru-RU`)
- Українська (`uk-UA`)
- Tiếng Việt (`vi-VN`)
- 简体中文 (`zh-CN`)
- 繁體中文 (`zh-TW`)

README documentation is available in [English](README.md) and [Simplified Chinese](README.zh-CN.md).

## Repository Layout

```text
.
├─ src/
│  ├─ routes/           # tab pages and route-level views
│  ├─ components/       # reusable UI components
│  ├─ lib/              # API bridge, stores, constants, shared types
│  └─ locales/          # i18n dictionaries
├─ public/              # static assets
├─ package.json         # Node scripts and dependencies
└─ README*.md           # project documentation
```

## API Contract

The frontend uses a single abstraction: `AppAPI` (`src/lib/api.ts`).

| Method            | Input               | Output                   | Description                            |
| ----------------- | ------------------- | ------------------------ | -------------------------------------- |
| `loadConfig`      | -                   | `Promise<AppConfig>`     | Load active global config.             |
| `saveConfig`      | `AppConfig`         | `Promise<void>`          | Persist global config payload.         |
| `resetConfig`     | -                   | `Promise<void>`          | Regenerate/reset config.               |
| `scanModules`     | `path?: string`     | `Promise<Module[]>`      | Discover modules and rules.            |
| `saveModules`     | `Module[]`          | `Promise<void>`          | Batch persist module rulesets.         |
| `saveModuleRules` | `moduleId`, `rules` | `Promise<void>`          | Persist one module ruleset.            |
| `getStorageUsage` | -                   | `Promise<StorageStatus>` | Read storage backend mode.             |
| `getSystemInfo`   | -                   | `Promise<SystemInfo>`    | Read runtime/system-level diagnostics. |
| `getDeviceStatus` | -                   | `Promise<DeviceInfo>`    | Read device profile metadata.          |
| `getVersion`      | -                   | `Promise<string>`        | Read module/app version.               |
| `openLink`        | `url: string`       | `Promise<void>`          | Open external URL.                     |
| `reboot`          | -                   | `Promise<void>`          | Trigger reboot action.                 |
| `readLogs`        | -                   | `Promise<string>`        | Read daemon log content.               |

Primary schemas (`src/lib/types.ts`):

- `AppConfig`: `moduledir`, `mountsource`, `partitions`, `overlay_mode`, and feature flags.
- `ModuleRules`: module default mode and per-path override map.
- `Module`: metadata, mount status, and policy state.
- `SystemInfo` / `DeviceInfo`: runtime observability payloads.

## Build and Development

Requirements:

- Node.js 20+
- pnpm 9+

Commands:

```bash
pnpm install
pnpm dev
pnpm build
pnpm preview
pnpm lint
```

`pnpm dev` uses `MockAPI` by default. To run the dev server against a real KernelSU bridge, start it with `VITE_USE_MOCK=false pnpm dev`.

## Adapter Notes

- Keep **error semantics structured** (code + message), avoid UI parsing of raw stderr text.
- Keep JSON payloads **backward compatible** when extending `AppConfig`, `Module`, or `SystemInfo`.
- Ensure command inputs are **escaped/sanitized** before shell execution.
- Prefer **idempotent save operations** for scripting and recovery flows.
- Keep daemon state/log paths readable for robust troubleshooting.

## License

Licensed under [Apache-2.0](LICENSE).
