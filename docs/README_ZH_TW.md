# Hybrid Mount

<img src="https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/main/icon.svg" align="right" width="120" />

![Language](https://img.shields.io/badge/Language-Rust-orange?style=flat-square&logo=rust)
![Platform](https://img.shields.io/badge/Platform-Android-green?style=flat-square&logo=android)
![License](https://img.shields.io/badge/License-Apache--2.0-blue?style=flat-square)
![Version](https://img.shields.io/github/v/tag/Hybrid-Mount/meta-hybrid_mount?label=Version&color=8A2BE2&style=flat-square)

Hybrid Mount 是面向 **KernelSU** 與 **APatch** 的掛載編排元模組。
它透過統一策略引擎，將模組檔案合併到 Android 分割區，並支援三種掛載後端：

- **OverlayFS** — 分層掛載，偏向廣泛相容性。
- **Magic Mount** — bind mount，適合直接路徑替換或回退場景。
- **Kasumi** — 基於 LKM 的路由，提供執行階段 hide、spoof 與 stealth 功能。

內建 **SolidJS WebUI**，提供圖形化管理、即時狀態監控與設定編輯。

發行套件分為三種版本。除非另有說明，本文預設描述 `full` 版本。

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
- [Kasumi](#kasumi)
- [策略參考](#策略參考)
- [CLI](#cli)
- [架構](#架構)
- [構建](#構建)
- [運維注意事項](#運維注意事項)
- [授權](#授權)

---

## 構建版本

| 版本 | 二進位 | WebUI | 守護行程 / CLI | Kasumi LKM | 適用場景 |
|------|--------|-------|----------------|------------|----------|
| **Full** | 是 | 是 | 是 | 是 | 需要 Kasumi 路由或 hide/spoof 功能的使用者。 |
| **Lite** | 是 | 是 | 是 | 否 | 需要 WebUI 與完整策略引擎，但不需要 LKM 級 stealth 功能的使用者。 |
| **Nano** | 是 | 否 | 否 | 否 | 只想透過設定檔控制掛載、不需要常駐守護行程或 WebUI 的使用者。 |

### Full

`full` 版本包含所有支援的掛載後端（OverlayFS、Magic Mount、Kasumi）、SolidJS WebUI、Unix socket 守護行程、HTTP/SSE、CLI 與 Kasumi LKM 資產。 使用 Cargo features `kasumi`（包含 `control-plane`）構建。

### Lite

`lite` 版本（`--no-default-features --features control-plane`）移除 Kasumi LKM 以及所有 Kasumi 相關功能，但保留 WebUI、守護行程、CLI、OverlayFS 與 Magic Mount。適合不需要 LKM 級 hide/spoof 能力，但仍需要圖形介面與完整策略引擎的環境。

### Nano

`nano` 版本（`--no-default-features`，無 Cargo features）是純設定檔驅動的構建。它移除 WebUI、守護行程、CLI 與控制面基礎設施，只保留啟動時讀取 `config.toml`、產生掛載計畫並執行的精簡二進位。

Nano 的預設模式為 `magic`。安裝時以音量鍵選擇後，會在受管理模組根目錄寫入 `overlay` 或 `magic` 標記檔；標記檔名以不區分大小寫的方式比對。啟動階段掛載完成後，Nano 二進位會結束，不保留常駐 Hybrid Mount 行程。

### 功能矩陣

| 功能 | Full | Lite | Nano |
|------|------|------|------|
| OverlayFS 後端 | 是 | 是 | 標記驅動 |
| Magic Mount 後端 | 是 | 是 | 是（預設） |
| Kasumi 後端 | 是 | 否 | 否 |
| WebUI | 是 | 是 | 否 |
| CLI | 是 | 是 | 否 |
| 守護行程 | 是 | 是 | 否 |
| 設定快取與執行階段套用 | 是 | 是 | 否 |
| Kasumi hide/spoof/stealth | 是 | 否 | 否 |
| LKM 自動載入 | 是 | 否 | 否 |
| Cargo features | `kasumi`（包含 `control-plane`） | 僅 `control-plane` | 無 |
| ZIP 體積（約） | ~4 MB | ~2 MB | ~1 MB |

## 特性

- **三種後端，一套策略引擎** — 可按路徑粒度分配 OverlayFS、Magic Mount 或 Kasumi。
- **可預期的規劃** — 衝突在計畫階段檢出，而不是在啟動時隨機暴露。
- **內建 WebUI** — 可管理模組、編輯設定、監控執行階段狀態；Full 版本可控制 Kasumi 功能。
- **Kasumi 執行階段整合** — 支援 LKM 自動載入、mirror 路由、mount 隱藏、maps/statfs spoof、UID 隱藏、uname spoof 與 kstat 規則。
- **設定快取** — 支援增量 patch 與立即套用。
- **恢復友善** — 會清理殘留執行階段檔案；設定錯誤時可透過 `api config-reset` 重設。
- **便於自動化** — 提供 JSON-over-Unix-socket 守護行程協定與 HTTP API。

---

## 快速開始

1. 在裝置上安裝 [KernelSU](https://kernelsu.org/) 或 [APatch](https://apatch.dev/)。
2. 從 [GitHub Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases) 下載 `full`、`lite` 或 `nano` ZIP。
3. 透過 root 管理器的模組安裝器刷入 ZIP。
4. 重新啟動。Hybrid Mount 會自動偵測環境並套用預設 overlay 策略。

```bash
# 檢查執行階段狀態
hybrid-mount daemon status

# 列出已偵測模組
hybrid-mount api modules-list
```

Full/Lite 版本可從 KernelSU 或 APatch 管理器的模組頁面開啟 WebUI。

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
| `kasumi` | Kasumi LKM | 需要明確 mirror 路由或執行階段 hide/spoof 功能的模組。 |
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
- Kasumi 控制面板：LKM 狀態、規則、功能開關與 spoof 設定（僅 Full）。

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
| `default_mode` | `overlay` \| `magic` \| `kasumi` | `overlay` | 全域預設掛載策略。 |
| `daemon_startup_mode` | `on-demand` \| `persistent` | `on-demand` | 守護行程啟動模式。 |
| `rules` | map | `{}` | 逐模組與逐路徑策略。 |

---

## Kasumi

Kasumi 是基於 LKM 的後端。除掛載路由外，還提供 hide 與 spoof 能力。啟用 `kasumi.enabled = true` 後，當掛載計畫含有 Kasumi 規則，或設定了 hidexattr、mount hide、maps spoof、statfs spoof、UID hiding、uname spoof、cmdline replacement、kstat/user hide 規則時，Kasumi 執行階段會被使用。

常用命令：

```bash
hybrid-mount kasumi status
hybrid-mount kasumi features
hybrid-mount kasumi list
hybrid-mount lkm status
hybrid-mount kasumi apply-config-runtime
hybrid-mount kasumi clear
```

---

## 策略參考

策略優先順序：

1. 路徑級覆寫：`rules.<module>.paths["<path>"]`
2. 模組級預設：`rules.<module>.default_mode`
3. 全域預設：`default_mode`

支援的模組標記檔包含 `disable`、`remove`、`skip_mount`、`mount_error`、`overlay`、`magic` 與 `.replace`。標記檔名以不區分大小寫方式比對；若同一目錄存在多個大小寫變體，清理流程會移除所有相符變體。

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
- `kasumi ...`：管理 Kasumi。
- `lkm load` / `lkm unload` / `lkm status`：管理 LKM。

---

## 架構

Hybrid Mount 的執行流程是：讀取 `config.toml`，掃描模組清單，依照路徑、模組與全域策略產生掛載計畫，再交由 OverlayFS、Magic Mount 或 Kasumi 執行器處理。執行器由 **typestate 狀態機**（`src/core/controller.rs`）驅動：`MountController<Init> → StorageReady → Planned → Executed`，每次狀態轉換代表一個管線階段。Full/Lite 版本會將執行階段狀態持久化，並透過守護行程提供 WebUI 與 CLI 存取。

主要目錄：

- `src/conf`：設定 schema、TOML 載入、CLI 定義與處理。
- `src/domain`：核心型別、規則與路徑比對。
- `src/core`：掃描、規劃、守護行程、API、啟動流程與狀態。
- `src/mount`：OverlayFS、Magic Mount 與 Kasumi 後端。
- `src/sys`：低階 mount syscall、LKM 與 Kasumi UAPI。
- `webui`：SolidJS WebUI 與 9 種語言的 i18n 檔案。
- `xtask`：構建與發行自動化。

---

## 構建

需求：

- Rust nightly（來自 `rust-toolchain.toml`）
- Android NDK r27+ 與 `cargo-ndk`
- Node.js 20+ 與 pnpm（用於 WebUI）

```bash
cargo run -p xtask -- build --release --flavor full
cargo run -p xtask -- build --release --flavor lite
cargo run -p xtask -- build --release --flavor nano
cargo run -p xtask -- build --release --skip-webui
./scripts/build-local.sh
cargo run -p xtask -- lint
cargo +nightly test
```

### CI 門禁與 feature flag 檢查

每次變更必須通過以下 CI 檢查：`cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all-targets --workspace`、WebUI `pnpm lint` + `pnpm test`，以及授權標頭檢查。`cargo clippy --all-features` 僅檢查 `full` 版本；也請確保 **lite**（`--no-default-features --features control-plane`）與 **nano**（`--no-default-features`）版本能夠編譯。涉及 Kasumi 的程式碼須置於 `#[cfg(feature = "kasumi")]` 之後；涉及 daemon/CLI/WebUI 的程式碼須置於 `#[cfg(feature = "control-plane")]` 之後。

---

## 運維注意事項

- 新安裝會自動偵測 `mountsource`；只有自動偵測失敗時才需要手動指定。
- 設定損壞時可執行 `hybrid-mount api config-reset` 重設，再逐步套用規則。
- 使用 `api config-patch --apply-runtime` 可立即套用部分設定變更。
- Full 版本的 Kasumi LKM 必須與目前核心相符；自動偵測錯誤時可使用 `lkm_kmi_override`。
- `kasumi clear` 會清除執行階段狀態並釋放核心連線；核心側既有規則可能直到重新載入 LKM 才會完全消失。

---

## 授權

本專案採用 [Apache-2.0](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/LICENSE) 授權。
