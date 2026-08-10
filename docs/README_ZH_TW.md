# Hybrid Mount

<img src="https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/main/icon.svg" align="right" width="120" />

![Language](https://img.shields.io/badge/Language-Rust-orange?style=flat-square&logo=rust)
![Platform](https://img.shields.io/badge/Platform-Android-green?style=flat-square&logo=android)
![License](https://img.shields.io/badge/License-GPL--3.0-blue?style=flat-square)
![Version](https://img.shields.io/github/v/tag/Hybrid-Mount/meta-hybrid_mount?label=Version&color=8A2BE2&style=flat-square)

Hybrid Mount 是面向 **KernelSU** 與 **APatch** 的掛載編排元模組。
它透過統一策略引擎，將模組檔案合併到 Android 分割區，並支援兩種掛載後端：

- **OverlayFS** — 分層掛載，偏向廣泛相容性。
- **Magic Mount** — 使用 bind mount 直接替換目標路徑。

內建 **SolidJS WebUI**，提供圖形化管理、即時狀態監控與設定編輯。

發行套件分為兩種版本。除非另有說明，本文預設描述 Lite 版本。

**[English](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/README.md)** &nbsp; **[简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH.md)** &nbsp; **[繁體中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH_TW.md)** &nbsp; **[日本語](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_JP.md)** &nbsp; **[Español](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ES.md)** &nbsp; **[Italiano](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_IT.md)** &nbsp; **[Русский](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_RU.md)** &nbsp; **[Українська](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_UK.md)** &nbsp; **[Tiếng Việt](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_VI.md)**

---

## 目錄

- [特性](#特性)
- [構建版本](#構建版本)
- [快速開始](#快速開始)
- [掛載模式](#掛載模式)
- [WebUI](#webui)
- [語言支援](#語言支援)
- [設定](#設定)
- [策略參考](#策略參考)
- [CLI](#cli)
- [架構](#架構)
- [構建](#構建)
- [授權](#授權)

---

## 構建版本

Hybrid Mount 發布為兩個版本（flavor），分別面向不同使用場景：

| 版本 | 二進位 | WebUI | 守護行程 / CLI | 適用場景 |
|------|--------|-------|---------------|----------|
| **Lite（預設）** | 是 | 是 | 是 | 預設發行版：WebUI、守護行程、CLI，以及 OverlayFS 與 Magic Mount 兩個後端。 |
| **Nano** | 是 | 否 | 否 | 只想透過設定檔控制掛載、不需要常駐守護行程或 WebUI 的使用者。 |

### Lite

Lite 是預設發行版，包含 SolidJS WebUI、Unix socket 守護行程（HTTP/SSE）、CLI，以及 OverlayFS 與 Magic Mount 兩個後端：

- 需要 WebUI 和完整策略引擎。
- 想要更小的下載體積，同時保留 WebUI 和守護行程管理介面。

Lite 構建僅使用 `control-plane` feature（`--no-default-features --features control-plane`）。

### Nano

`nano` 版本（`--no-default-features`，無 Cargo features）是純設定檔驅動的構建。它移除 WebUI、守護行程、CLI 與控制面基礎設施，只保留啟動時讀取 `config.toml`、產生掛載計畫並執行的精簡二進位。

Nano 的預設模式為 `magic`。安裝時以音量鍵選擇後，會在受管理模組根目錄寫入 `overlay` 或 `magic` 標記檔；標記檔名必須使用完全相同的小寫拼寫。啟動階段掛載完成後，Nano 二進位會結束，不保留常駐 Hybrid Mount 行程。

### 功能矩陣

| 功能 | Lite | Nano |
| ------ | ------ | ------ |
| OverlayFS 後端 | 是 | 標記驅動 |
| Magic Mount 後端 | 是 | 是（預設） |
| WebUI | 是 | 否 |
| CLI | 是 | 否 |
| 守護行程 | 是 | 否 |
| 執行階段設定套用 | 是 | 否 |
| Cargo features | 僅 `control-plane` | 無 |
| ZIP 體積（約） | ~2 MB | ~1 MB |

## 特性

- **可預期的規劃** — 衝突在計畫階段檢出，而不是在啟動時隨機暴露。
- **執行階段設定更新** — 通過嚴格驗證的設定 patch 可持久化並立即套用。
- **明確失敗回報** — 無效狀態或設定會立即報錯；重設設定必須明確呼叫 `api config-reset`。
- **便於自動化** — 提供 JSON-over-Unix-socket 守護行程協定與 HTTP API。

---

## 快速開始

1. 在裝置上安裝 [KernelSU](https://kernelsu.org/) 或 [APatch](https://apatch.dev/)。
2. 從 [GitHub Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases) 下載 `lite` 或 `nano` ZIP。
3. 透過 root 管理器的模組安裝器刷入 ZIP。
4. 首次安裝時直接選擇預設掛載模式：音量加選擇 OverlayFS，音量減選擇 Magic Mount，10 秒無操作則選擇 OverlayFS。這是安裝器唯一的互動步驟；Nano 會跳過此步驟。
5. 重新啟動。Hybrid Mount 會自動偵測環境並套用所選策略。

```bash
# 檢查執行階段狀態
hybrid-mount daemon status

# 列出已偵測模組
hybrid-mount api modules-list
```

Lite 版本可從 KernelSU 或 APatch 管理器的模組頁面開啟 WebUI。

### 調整模組掛載模式

```toml
# /data/adb/hybrid-mount/config.toml
[rules.my_module]
default_mode = "magic"

[rules.my_module.paths]
"system/bin/problematic_binary" = "ignore"
```

---

## 掛載模式

| 模式 | 後端 | 適用場景 |
|------|------|----------|
| `overlay` | OverlayFS | 無衝突新增或替換檔案的模組。預設模式。 |
| `magic` | Bind mount | 需要逐檔直接替換的模組。 |
| `ignore` | 無 | 排除指定路徑，不進行掛載處理。 |

OverlayFS 的 upper/work 層可使用 `ext4`（預設，持久化）或 `tmpfs`（揮發、較輕量）。

---

## WebUI

Hybrid Mount 內建基於 SolidJS 的 WebUI，由守護行程透過本機 TCP socket 提供 HTTP/SSE。CLI 與自動化客戶端透過 Unix socket 通訊。

WebUI 可在 KernelSU 或 APatch 管理器內嵌 WebView 中直接開啟，不需要在裝置上安裝額外瀏覽器。

主要功能：

- 狀態面板：掛載統計、分割區、儲存模式、守護行程狀態。
- 模組管理：列出模組與有效掛載模式，並可互動修改。
- 設定編輯器：編輯並驗證 `config.toml`，包含逐模組路徑規則。

### 語言支援

WebUI 目前提供以下 locale：

- English (`en-US`，預設)
- Español (`es-ES`)
- Italiano (`it-IT`)
- 日本語 (`ja-JP`)
- Русский (`ru-RU`)
- Українська (`uk-UA`)
- Tiếng Việt (`vi-VN`)
- 简体中文 (`zh-CN`)
- 繁體中文 (`zh-TW`)

README 文件提供 [English](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/README.md)、[简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH.md)、[繁體中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH_TW.md)、[日本語](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_JP.md)、[Español](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ES.md)、[Italiano](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_IT.md)、[Русский](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_RU.md)、[Українська](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_UK.md) 與 [Tiếng Việt](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_VI.md) 版本。

---

## 設定

預設路徑：`/data/adb/hybrid-mount/config.toml`。

| 欄位 | 型別 | 預設值 | 說明 |
| --- | --- | --- | --- |
| `moduledir` | string | `/data/adb/modules` | 模組來源目錄。 |
| `mountsource` | string | 自動偵測 | 執行環境標記（`KSU`、`APatch`）。 |
| `overlay_mode` | `ext4` \| `tmpfs` | `ext4` | Overlay upper/work 儲存模式。 |
| `disable_umount` | bool | `false` | 跳過 umount，僅供除錯。 |
| `rules` | map | `{}` | 逐模組與逐路徑策略。 |

---

## 策略參考

策略優先順序：

1. 路徑級覆寫：`rules.<module>.paths["<path>"]`
2. 模組級預設：`rules.<module>.default_mode`
3. 全域預設：`default_mode`

支援的模組標記檔包含 `disable`、`remove`、`skip_mount`、`overlay`、`magic` 與 `.replace`。標記檔名嚴格區分大小寫，必須使用列出的精確名稱。

---

## CLI

```bash
hybrid-mount [OPTIONS] [COMMAND]
```

常用子命令包含：

- `gen-config`：產生預設設定檔。
- `logs`：輸出近期守護行程日誌。
- `api config-get` / `api config-set` / `api config-patch` / `api config-reset`：管理設定。
- `api modules-list` / `api modules-apply`：查詢與套用模組策略。
- `daemon launch` / `daemon serve` / `daemon status` / `daemon stop`：管理守護行程。

---

## 架構

主要目錄：

- `src/conf`：設定 schema、TOML 載入、CLI 定義與處理。
- `src/domain`：核心型別、規則與路徑比對。
- `src/core`：掃描、規劃、守護行程、API、啟動流程與狀態。
- `webui`：SolidJS WebUI 與 9 種語言的 i18n 檔案。
- `xtask`：構建與發行自動化。

---

## 構建

需求：

- Rust nightly（來自 `rust-toolchain.toml`）
- Android NDK r27+ 與 `cargo-ndk`
- Node.js 20+ 與 pnpm（用於 WebUI）

```bash
cargo run -p xtask -- build --release --flavor lite
cargo run -p xtask -- build --release --flavor nano
cargo run -p xtask -- build --release --skip-webui
./scripts/build-local.sh
cargo run -p xtask -- lint
cargo +nightly test
```

### CI 門禁與 feature flag 檢查

---

## 授權

本專案採用 [GPL-3.0](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/LICENSE) 授權。
