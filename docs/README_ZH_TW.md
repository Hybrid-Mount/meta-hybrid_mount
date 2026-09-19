# Hybrid Mount

<img src="../icon.svg" alt="Hybrid Mount logo" align="right" width="120" />

Hybrid Mount 是面向 KernelSU 與 APatch 的混合掛載元模組。它會在啟動階段掃描其他模組，依照全域、模組和路徑規則，為每一項選擇 OverlayFS、Magic Mount、VFS 或忽略，並且始終把模組來源目錄視為唯讀輸入。

## 功能

- OverlayFS、Magic Mount 與 VFS 可依模組、依路徑混用。
- 路徑規則優先於模組預設值，模組預設值優先於全域預設值。
- OverlayFS 支援 tmpfs 與 ext4 兩種儲存模式。
- ext4 staging 在 KernelSU 使用官方 ioctl 隱藏 sysfs 節點；在 APatch 等非 KSU 環境預設使用隨附的 LKM 相容後備方案。
- Magic Mount 支援檔案、目錄、符號連結、`.replace` 和 whiteout 語意。
- VFS 透過 keyring 將注入規則下發給 Hybrid Mount 自有的 VFS 子系統（`hybridmount` 模組）。這是獨立實作，不與 NoMount 核心或其 nm CLI 互通。發佈包同時提供原始碼與每個受支援 Android/GKI 目標的 arm64 預編譯模組，核心未內建時由啟動流程自動載入；仍不可用時依 `vfs_strict` 降級。VFS 不是真實掛載。
- WebUI 提供 MD3（預設）與 Miuix 兩套介面。
- 支援 arm64、armv7 與 x86_64，安裝程式會自動選擇對應的二進位檔案。

## 安裝

從 [Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases) 下載 ZIP，並在 KernelSU 或 APatch 管理器中安裝。首次安裝可使用音量鍵選擇預設後端；升級時會保留 `/data/adb/hybrid-mount/config.toml`。

## 設定

預設設定：

```toml
moduledir = "/data/adb/modules"
overlay_mode = "ext4" # ext4 | tmpfs
disable_umount = false
default_mode = "overlay" # overlay | magic | vfs

[rules.example_module]
default_mode = "magic"

[rules.example_module.paths]
"system/etc/hosts" = "overlay"
```

規則路徑相對於模組根目錄書寫。模組層級和路徑層級規則仍可使用 `ignore`；全域預設後端接受 `overlay`、`magic` 或 `vfs`。VFS 是注入路徑，不是真實掛載。檔案、類型或 `.replace` 衝突會在啟動規劃階段直接報錯。設定修改會在重新啟動後生效。

這套路由不會改變專案現有的 `CONFIG_TMPFS_XATTR` 能力判斷。KernelSU 安裝時會刪除模組中的整個 `lkm/` 目錄，執行時只使用官方 `NukeExt4Sysfs` ioctl；APatch 等非 KSU 安裝會保留 LKM，並在 ext4 staging 掛載後預設嘗試使用。隨附的 `.ko` 僅支援 aarch64；自動選擇要求核心系列和 Android/GKI 標籤完全相符，未知組合會直接拒絕，但預編譯 LKM 仍必須在對應的實機上驗證 ABI 相容性。若裝置在 `insmod` 期間當機，持久熔斷標記會阻止下次啟動再次載入 LKM，同時保留 Hybrid Mount 的其他功能。支援矩陣、校驗值、來源與授權請參閱 [`module/lkm/README.md`](../module/lkm/README.md)。

## VFS 後端

VFS 是 Hybrid Mount 自有的核心端注入路徑，由 `hybridmount` 模組經由 keyring 驅動。這是獨立實作，不與 NoMount 互通。

**如何識別 Provider。** 啟動決策只看對核心 key type `hybridmount` 的一次唯讀探測：只要它回應受支援的版本，Provider 即可使用。`vfs-doctor` 另外負責判斷它以何種方式存在——出現在 `/proc/modules` 中，代表由可載入模組註冊；有 `/sys/module/hybridmount` 目錄但沒有上述項目，代表已編譯進核心映像；兩者皆無，代表本機沒有 Provider。探測是唯讀的，因此 `status` 與 `vfs-doctor` 都不會觸發 `insmod`。

**啟動邏輯。** 若 key type 回應受支援的版本，Provider 即被綁定，不會載入任何東西。若沒有規則選擇 VFS，隨附模組同樣不會載入。若確有規則選擇 VFS 而探測沒有回應，啟動流程會挑選與核心線及 Android/GKI 標籤完全相符的隨附模組，載入後重新探測；仍不可用時，所有 `vfs` 規則降級為 `ignore`，`vfs_strict = true` 時則啟動失敗。載入發生在掛載計畫建構之前，因為規劃階段會在 Provider 沒有回應時把 `vfs` 規則改寫為 `ignore`，執行器隨後就會提前返回。熔斷標記在 `insmod` 前寫入，嘗試返回時清除，因此只有核心崩潰才會把它留下；下次啟動將拒絕自動重試，直到手動刪除該標記。

**把 VFS 整合進核心。** 發佈包為每個受支援的 Android/GKI 目標都提供 aarch64 預編譯模組並自動載入，因此這些核心無需任何整合步驟。當你想避免 `insmod`，或你的核心線沒有對應預編譯模組時，可以將它內建進核心。在核心原始碼樹根目錄執行：

```sh
sh /path/to/metamodule/module/vfs/setup.sh
```

這會把原始碼複製到 `fs/hybridmount/`，並加入 `fs/Makefile` 與 `fs/Kconfig`；啟用 `CONFIG_HYBRIDMOUNT=y` 表示內建，`=m` 表示編譯為模組。`--cleanup` 會還原全部變更。已整合 NoMount 的核心樹會被拒絕：兩種實作都會劫持 inode 操作，而由於它們註冊的 key type 不同，核心不會阻止二者並存。

**診斷。** `/data/adb/modules/hybrid_mount/hybrid-mount vfs-doctor` 會報告存在狀態、key type 回應的版本、受支援的版本，以及 Provider 無法使用時的原因。

## 意見回饋

安裝或回報問題前，請閱讀[使用須知](../USAGE_NOTICE.md)。回報時請附上 KernelSU/APatch bugreport、模組版本與可重現步驟，可透過 [GitHub Issues](https://github.com/Hybrid-Mount/meta-hybrid_mount/issues) 或 [Telegram 群組](https://t.me/hybridmountchat)聯絡我們。

## 語言 / Languages

- [English](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/README.md)
- [Español](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_ES.md)
- [Français](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_FR.md)
- [Bahasa Indonesia](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_ID.md)
- [Italiano](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_IT.md)
- [日本語](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_JA.md)
- [Русский](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_RU.md)
- [Türkçe](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_TR.md)
- [Українська](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_UK.md)
- [Tiếng Việt](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_VI.md)
- [简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_ZH.md)
- [繁體中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_ZH_TW.md)

## 鳴謝

- 感謝 [Anatdx](https://github.com/Anatdx)
- 感謝 [Tools-cx-app](https://github.com/Tools-cx-app)
- 感謝 [KernelSU](https://github.com/tiann/KernelSU)
- 感謝 [5ec1cff 的 MKSU](https://github.com/5ec1cff/KernelSU)
- 感謝 [ReSukiSU](https://github.com/ReSukiSU/ReSukiSU)
- 感謝 [meta-magic_mount-rs](https://github.com/Tools-cx-app/meta-magic_mount-rs)
- 感謝 [NoMount](https://github.com/maxsteeel/nomount)

## 授權條款

- 核心（Rust、module 指令碼）：GPL-3.0-only（參閱 [`LICENSE`](../LICENSE)）。
- WebUI：Apache-2.0（參閱 [`webui/LICENSE`](../webui/LICENSE)）。
- 選用的 ext4 sysfs LKM（原始碼與預編譯 `.ko`）：GPL-2.0-only，源自 [Mountify](https://github.com/backslashxx/mountify)；參閱 [`module/lkm/README.md`](../module/lkm/README.md) 與 [`module/lkm/src/LICENSE`](../module/lkm/src/LICENSE)。
- VFS 子系統（`hybridmount` 模組）：GPL-2.0-only，fork 自 [NoMount](https://github.com/maxsteeel/nomount)；參閱 [`module/vfs/README.md`](../module/vfs/README.md)、[`module/vfs/src/LICENSE`](../module/vfs/src/LICENSE) 與 [THIRD_PARTY.md](../THIRD_PARTY.md)。
