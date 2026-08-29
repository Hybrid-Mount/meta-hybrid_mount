# Hybrid Mount

Hybrid Mount は、KernelSU と APatch 向けのハイブリッドマウントメタモジュールです。起動時に他のモジュールをスキャンし、グローバル、モジュール、パスの各ルールに基づいて、項目ごとに OverlayFS、Magic Mount、または無視を選択します。モジュールのソースディレクトリは常に読み取り専用の入力として扱われます。

## 機能

- OverlayFS と Magic Mount をモジュール単位およびパス単位で併用できます。
- パスルールはモジュールのデフォルトより優先され、モジュールのデフォルトはグローバルのデフォルトより優先されます。
- OverlayFS は tmpfs と ext4 の両方のストレージモードに対応します。
- ext4 ステージングでは、KernelSU は公式 ioctl で sysfs ノードを非表示にします。APatch などの非 KSU 環境では、同梱の互換 LKM をデフォルトで使用します。
- Magic Mount は、ファイル、ディレクトリ、シンボリックリンク、`.replace`、whiteout セマンティクスに対応します。
- WebUI には MD3（デフォルト）と Miuix の2種類のインターフェースがあります。
- arm64、armv7、x86_64 に対応し、インストーラーが適切なバイナリを自動的に選択します。

## インストール

[Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases) から ZIP をダウンロードし、KernelSU または APatch マネージャーでインストールしてください。初回インストール時は、音量キーでデフォルトのバックエンドを選択できます。アップデート時も `/data/adb/hybrid-mount/config.toml` は保持されます。

## 設定

デフォルト設定：

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

ルールのパスはモジュールルートからの相対パスで記述します。モジュール単位とパス単位のルールでは `ignore` も使用できますが、グローバルのデフォルトバックエンドに指定できるのは `overlay` または `magic` のみです。同じファイルパスを両方のマウントバックエンドに割り当てることはできません。通常のディレクトリは両方のバックエンドで構造ノードとして共有できますが、ファイル、種類、`.replace` の競合がある場合は、起動時のプランニング段階で直ちにエラーになります。設定の変更は再起動後に反映されます。

この振り分けによって、プロジェクト既存の `CONFIG_TMPFS_XATTR` 機能判定が変更されることはありません。KernelSU では、インストール時にモジュール内の `lkm/` ディレクトリ全体を削除し、実行時には公式の `NukeExt4Sysfs` ioctl のみを使用します。APatch などの非 KSU 環境では LKM を保持し、ext4 ステージングのマウント後にデフォルトで使用を試みます。同梱の `.ko` は aarch64 のみに対応しています。自動選択にはカーネル系列と Android/GKI タグの完全一致が必要で、不明な組み合わせは拒否されます。ビルド済み LKM についても、対応する実機で ABI 互換性を検証する必要があります。`insmod` 中に端末がクラッシュした場合、永続的なサーキットブレーカーマーカーによって次回起動時の LKM 再読み込みを防ぎ、Hybrid Mount のその他の機能は維持されます。対応表、チェックサム、ソース、ライセンスについては [`module/lkm/README.md`](../module/lkm/README.md) を参照してください。

## フィードバック

インストールまたは問題を報告する前に、[使用上の注意](../USAGE_NOTICE.md)をお読みください。KernelSU/APatch の bugreport、モジュールのバージョン、再現手順を添えてください。[GitHub Issues](https://github.com/Hybrid-Mount/meta-hybrid_mount/issues) または [Telegram グループ](https://t.me/hybridmountchat)からお問い合わせいただけます。

## 言語 / Languages

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

## ライセンス

- コア（Rust およびモジュールスクリプト）：GPL-3.0-only（[`LICENSE`](../LICENSE) を参照）。
- WebUI：Apache-2.0（[`webui/LICENSE`](../webui/LICENSE) を参照）。
- オプションの ext4 sysfs LKM（ソースおよびビルド済み `.ko`）：[Mountify](https://github.com/backslashxx/mountify) 由来の GPL-2.0-only。詳しくは [`module/lkm/README.md`](../module/lkm/README.md) と [`module/lkm/src/LICENSE`](../module/lkm/src/LICENSE) を参照してください。
