# Hybrid Mount

<img src="../icon.svg" alt="Hybrid Mount logo" align="right" width="120" />

Hybrid Mount は、KernelSU と APatch 向けのハイブリッドマウントメタモジュールです。起動時に他のモジュールをスキャンし、グローバル、モジュール、パスの各ルールに基づいて、項目ごとに OverlayFS、Magic Mount、VFS、または無視を選択します。モジュールのソースディレクトリは常に読み取り専用の入力として扱われます。

## 機能

- OverlayFS、Magic Mount、VFS をモジュール単位およびパス単位で併用できます。
- パスルールはモジュールのデフォルトより優先され、モジュールのデフォルトはグローバルのデフォルトより優先されます。
- OverlayFS は tmpfs と ext4 の両方のストレージモードに対応します。
- ext4 ステージングでは、KernelSU は公式 ioctl で sysfs ノードを非表示にします。APatch などの非 KSU 環境では、同梱の互換 LKM をデフォルトで使用します。
- Magic Mount は、ファイル、ディレクトリ、シンボリックリンク、`.replace`、whiteout セマンティクスに対応します。
- VFS は keyring 経由で Hybrid Mount 独自の VFS サブシステム（`hybridmount` モジュール）に注入ルールを送ります。独立実装であり、NoMount のカーネルやその nm CLI とは相互運用しません。リリースにはソースと、対応する Android/GKI ターゲットごとの arm64 プリコンパイル済みモジュールが含まれ、カーネルに組み込まれていない場合は起動時に読み込まれます。失敗した場合は `vfs_strict` に従って縮退します。VFS は実際のマウントではありません。
- WebUI には MD3（デフォルト）と Miuix の2種類のインターフェースがあります。
- arm64、armv7、x86_64、riscv64 に対応し、インストーラーが適切なバイナリを自動的に選択します。riscv64 のビルドには Android NDK r27 以降が必要です。

## インストール

[Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases) から ZIP をダウンロードし、KernelSU または APatch マネージャーでインストールしてください。初回インストール時は、音量キーでデフォルトのバックエンドを選択できます。アップデート時も `/data/adb/hybrid-mount/config.toml` は保持されます。

## 設定

デフォルト設定：

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

ルールのパスはモジュールルートからの相対パスで記述します。モジュール単位とパス単位のルールでは `ignore` も使用でき、グローバルのデフォルトバックエンドには `overlay`、`magic`、`vfs` を指定できます。VFS は実マウントではなく注入経路です。ファイル、種類、`.replace` の競合は起動時のプランニング段階で直ちにエラーになります。設定の変更は再起動後に反映されます。

この振り分けによって、プロジェクト既存の `CONFIG_TMPFS_XATTR` 機能判定が変更されることはありません。KernelSU では、インストール時にモジュール内の `lkm/` ディレクトリ全体を削除し、実行時には公式の `NukeExt4Sysfs` ioctl のみを使用します。APatch などの非 KSU 環境では LKM を保持し、ext4 ステージングのマウント後にデフォルトで使用を試みます。同梱の `.ko` は aarch64 のみに対応しています。自動選択にはカーネル系列と Android/GKI タグの完全一致が必要で、不明な組み合わせは拒否されます。ビルド済み LKM についても、対応する実機で ABI 互換性を検証する必要があります。`insmod` 中に端末がクラッシュした場合、永続的なサーキットブレーカーマーカーによって次回起動時の LKM 再読み込みを防ぎ、Hybrid Mount のその他の機能は維持されます。対応表、チェックサム、ソース、ライセンスについては [`module/lkm/README.md`](../module/lkm/README.md) を参照してください。

## VFS バックエンド

VFS は Hybrid Mount 独自のカーネル側注入パスであり、`hybridmount` モジュールが keyring 経由で操作します。独立した実装であり、NoMount とは相互運用しません。

**Provider の識別方法。** 起動時の判断は、カーネルの key type `hybridmount` に対する読み取り専用のプローブだけに基づきます。対応バージョンを返せば、その Provider は使用可能です。これとは別に `vfs-doctor` が、どのような形で存在するかを判定します。`/proc/modules` にエントリがあればローダブルモジュールが登録したもので、そのエントリがなく `/sys/module/hybridmount` ディレクトリがあればカーネルイメージに組み込まれており、どちらもなければ Provider は存在しません。プローブは読み取り専用のため、`status` と `vfs-doctor` が `insmod` を引き起こすことはありません。

**起動ロジック。** key type が対応バージョンを返せば、Provider がバインドされ、何も読み込みません。VFS を選択するルールがなければ、同梱モジュールも読み込みません。プローブが応答しない状態で VFS を選択するルールがある場合、パイプラインはカーネル系列と Android/GKI タグが完全一致する同梱モジュールを選んで読み込み、再度プローブします。それでも使用できなければ、すべての `vfs` ルールは `ignore` に縮退し、`vfs_strict = true` の場合は起動に失敗します。読み込みはマウントプランの構築前に実行されます。プランニング段階では Provider が応答しない間 `vfs` ルールを `ignore` に書き換えるため、実行側がそのまま早期リターンしてしまうからです。サーキットブレーカーマーカーは `insmod` の前に書き込まれ、試行が戻ると消去されるので、カーネルクラッシュの場合にだけ残ります。次回起動では、このマーカーを手動で削除するまで自動再試行を拒否します。

**VFS をカーネルに統合する。** リリースには対応するすべての Android/GKI ターゲット向けの aarch64 プリコンパイル済みモジュールが含まれ、自動的に読み込まれるため、これらのカーネルに統合作業は不要です。`insmod` を避けたい場合や、お使いのカーネル系列にプリコンパイル済みモジュールがない場合は、カーネルに組み込んでビルドしてください。カーネルツリーのルートで次を実行します。

```sh
curl -LSs "https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/dev/module/vfs/setup.sh" | bash
```

`--cleanup`:

```sh
curl -LSs "https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/dev/module/vfs/setup.sh" | bash -s -- --cleanup
```

ソースが `fs/hybridmount/` にコピーされ、`fs/Makefile` と `fs/Kconfig` に追加されます。組み込む場合は `CONFIG_HYBRIDMOUNT=y`、モジュールとしてビルドする場合は `=m` を有効にします。`bash -s -- --cleanup` はすべての変更を元に戻します。すでに NoMount を統合しているツリーは拒否されます。両実装はどちらも inode 操作を乗っ取り、登録する key type が異なるため、カーネルはこれらの共存を止められません。

**診断。** `/data/adb/modules/hybrid_mount/hybrid-mount vfs-doctor` は、存在状態、key type が返したバージョン、対応バージョン、そして Provider が使用できない場合はその理由を報告します。

## フィードバック

インストールまたは問題を報告する前に、[使用上の注意](../USAGE_NOTICE.md)をお読みください。KernelSU/APatch の bugreport、モジュールのバージョン、再現手順を添えてください。[GitHub Issues](https://github.com/Hybrid-Mount/meta-hybrid_mount/issues) または [Telegram グループ](https://t.me/hybridmountchat)からお問い合わせいただけます。

## 言語 / Languages

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

## 謝辞

- [Anatdx](https://github.com/Anatdx) に感謝します。
- [Tools-cx-app](https://github.com/Tools-cx-app) に感謝します。
- [KernelSU](https://github.com/tiann/KernelSU) に感謝します。
- [5ec1cff 氏の MKSU](https://github.com/5ec1cff/KernelSU) に感謝します。
- [ReSukiSU](https://github.com/ReSukiSU/ReSukiSU) に感謝します。
- [meta-magic_mount-rs](https://github.com/Tools-cx-app/meta-magic_mount-rs) に感謝します。
- [NoMount](https://github.com/maxsteeel/nomount) に感謝します。

## ライセンス

- コア（Rust およびモジュールスクリプト）：GPL-3.0-only（[`LICENSE`](../LICENSE) を参照）。
- WebUI：Apache-2.0（[`webui/LICENSE`](../webui/LICENSE) を参照）。
- オプションの ext4 sysfs LKM（ソースおよびビルド済み `.ko`）：[Mountify](https://github.com/backslashxx/mountify) 由来の GPL-2.0-only。詳しくは [`module/lkm/README.md`](../module/lkm/README.md) と [`module/lkm/src/LICENSE`](../module/lkm/src/LICENSE) を参照してください。
- VFS サブシステム（`hybridmount` モジュール）：[NoMount](https://github.com/maxsteeel/nomount) 由来の GPL-2.0-only。詳しくは [`module/vfs/README.md`](../module/vfs/README.md)、[`module/vfs/src/LICENSE`](../module/vfs/src/LICENSE)、[THIRD_PARTY.md](../THIRD_PARTY.md) を参照してください。
