# Hybrid Mount

Hybrid Mount 是面向 KernelSU 與 APatch 的混合掛載元模組。它會在啟動階段掃描其他模組，依照全域、模組和路徑規則，為每一項選擇 OverlayFS、Magic Mount 或忽略，並且始終把模組來源目錄視為唯讀輸入。

## 功能

- OverlayFS 與 Magic Mount 可依模組、依路徑混用。
- 路徑規則優先於模組預設值，模組預設值優先於全域預設值。
- OverlayFS 支援 tmpfs 與 ext4 兩種儲存模式。
- ext4 staging 在 KernelSU 使用官方 ioctl 隱藏 sysfs 節點；在 APatch 等非 KSU 環境預設使用隨附的 LKM 相容後備方案。
- Magic Mount 支援檔案、目錄、符號連結、`.replace` 和 whiteout 語意。
- WebUI 提供 MD3（預設）與 Miuix 兩套介面。
- 支援 arm64、armv7 與 x86_64，安裝程式會自動選擇對應的二進位檔案。

## 安裝

從 [Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases) 下載 ZIP，並在 KernelSU 或 APatch 管理器中安裝。首次安裝可使用音量鍵選擇預設後端；升級時會保留 `/data/adb/hybrid-mount/config.toml`。

## 設定

預設設定：

```toml
moduledir = "/data/adb/modules"
mountsource = "KSU"
overlay_mode = "ext4" # ext4 | tmpfs
disable_umount = false
default_mode = "overlay" # overlay | magic

[rules.example_module]
default_mode = "magic"

[rules.example_module.paths]
"system/etc/hosts" = "overlay"
```

規則路徑相對於模組根目錄書寫。模組層級和路徑層級規則仍可使用 `ignore`；全域預設後端只接受 `overlay` 或 `magic`。同一個檔案路徑不能同時交給兩個掛載後端；一般目錄可以作為兩個後端共用的結構節點，檔案、類型或 `.replace` 衝突會在啟動規劃階段直接報錯。設定修改會在重新啟動後生效。

這套路由不會改變專案現有的 `CONFIG_TMPFS_XATTR` 能力判斷。KernelSU 安裝時會刪除模組中的整個 `lkm/` 目錄，執行時只使用官方 `NukeExt4Sysfs` ioctl；APatch 等非 KSU 安裝會保留 LKM，並在 ext4 staging 掛載後預設嘗試使用。隨附的 `.ko` 僅支援 aarch64；自動選擇要求核心系列和 Android/GKI 標籤完全相符，未知組合會直接拒絕，但預編譯 LKM 仍必須在對應的實機上驗證 ABI 相容性。若裝置在 `insmod` 期間當機，持久熔斷標記會阻止下次啟動再次載入 LKM，同時保留 Hybrid Mount 的其他功能。支援矩陣、校驗值、來源與授權請參閱 [`module/lkm/README.md`](../module/lkm/README.md)。

## 意見回饋

安裝或回報問題前，請閱讀[使用須知](../USAGE_NOTICE.md)。回報時請附上 KernelSU/APatch bugreport、模組版本與可重現步驟，可透過 [GitHub Issues](https://github.com/Hybrid-Mount/meta-hybrid_mount/issues) 或 [Telegram 群組](https://t.me/hybridmountchat)聯絡我們。

## 語言 / Languages

- [English](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_EN.md)
- [Español](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_ES.md)
- [Bahasa Indonesia](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_ID.md)
- [Italiano](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_IT.md)
- [日本語](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_JA.md)
- [Русский](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_RU.md)
- [Türkçe](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_TR.md)
- [Українська](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_UK.md)
- [Tiếng Việt](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_VI.md)
- [简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/README.md)
- [繁體中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_ZH_TW.md)

## 授權條款

- 核心（Rust、module 指令碼）：GPL-3.0-only（參閱 [`LICENSE`](../LICENSE)）。
- WebUI：Apache-2.0（參閱 [`webui/LICENSE`](../webui/LICENSE)）。
- 選用的 ext4 sysfs LKM（原始碼與預編譯 `.ko`）：GPL-2.0-only，源自 [Mountify](https://github.com/backslashxx/mountify)；參閱 [`module/lkm/README.md`](../module/lkm/README.md) 與 [`module/lkm/src/LICENSE`](../module/lkm/src/LICENSE)。
