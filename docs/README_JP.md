# Hybrid Mount

<img src="https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/main/icon.svg" align="right" width="120" />

![Language](https://img.shields.io/badge/Language-Rust-orange?style=flat-square&logo=rust)
![Platform](https://img.shields.io/badge/Platform-Android-green?style=flat-square&logo=android)
![License](https://img.shields.io/badge/License-GPL--3.0-blue?style=flat-square)
![Version](https://img.shields.io/github/v/tag/Hybrid-Mount/meta-hybrid_mount?label=Version&color=8A2BE2&style=flat-square)

Hybrid Mount は、**KernelSU** と **APatch** 向けのマウント統合メタモジュールです。
統一されたポリシーエンジンにより、2つのマウントバックエンドを通じてモジュールファイルをAndroidパーティションに統合します：

- **OverlayFS** — 広範な互換性のためのレイヤードマウント。
- **Magic Mount** — 直接パス置換またはフォールバックのためのバインドマウント。

内蔵の **SolidJS WebUI** が、グラフィカルな管理、ライブ状態監視、設定編集を提供します。

リリースは2つのフレーバーで公開されています — 詳細は [ビルドフレーバー](#ビルドフレーバー) を参照してください。特に明記しない限り、以降の説明は `lite` ビルドについてのものです。

**[English](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/README.md)** &nbsp; **[简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH.md)** &nbsp; **[繁體中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH_TW.md)** &nbsp; **[日本語](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_JP.md)** &nbsp; **[Español](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ES.md)** &nbsp; **[Italiano](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_IT.md)** &nbsp; **[Русский](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_RU.md)** &nbsp; **[Українська](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_UK.md)** &nbsp; **[Tiếng Việt](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_VI.md)**

---

## 目次

- [特徴](#特徴)
- [ビルドフレーバー](#ビルドフレーバー)
- [クイックスタート](#クイックスタート)
- [マウントモード](#マウントモード)
- [WebUI](#webui)
- [対応言語](#対応言語)
- [設定](#設定)
- [ポリシーリファレンス](#ポリシーリファレンス)
- [CLI](#cli)
- [アーキテクチャ](#アーキテクチャ)
- [ビルド](#ビルド)
- [運用上の注意](#運用上の注意)
- [ライセンス](#ライセンス)

---

## ビルドフレーバー

Hybrid Mount は、それぞれ異なるユースケースに対応する2つのフレーバーでリリースされています：

| フレーバー | バイナリ | WebUI | デーモン/CLI | 用途 |
|-----------|---------|-------|-------------|-----------|
| **Lite** | あり | あり | あり | WebUI と完全なポリシーエンジンが必要だが、LKMベースのステルス機能は不要なユーザー。 |
| **Nano** | あり | なし | なし | 設定ファイルのみでマウント統合を行いたいミニマリスト — デーモン、WebUI、CLI不要。 |

### Lite

`lite` フレーバーはデフォルトのビルドです。WebUI、デーモン、CLI、OverlayFSとMagic Mountの両バックエンドを維持しています。Liteを選ぶ理由：

- カーネルが外部LKMのロードをサポートしていない。
- ランタイム hide/spoof 機能が不要。
- WebUIとデーモン管理インターフェースを維持しつつ、ダウンロードサイズを抑えたい。

Liteビルドは `control-plane` フィーチャーセット（`--no-default-features --features control-plane`）を使用します。

### Nano

`nano` フレーバーは**設定ファイル専用**のビルドです（`--no-default-features` — Cargo features なし）。WebUI、デーモン、CLI、すべての制御基盤を除外しています。残るのは、`config.toml` を読み込み、マウント計画を生成し、実行して終了する小さなバイナリです。主な特徴：

- **デーモンなし** — バックグラウンドプロセス、ソケット、WebUI、CLIサブコマンドはありません。
- **WebUIなし** — `webroot/`、`launcher.png`、`service.sh` アセットはパッケージから削除されます。
- **マウント専用動作** — バイナリは起動時に実行され、設定に従ってすべてをマウントし、終了します。
- **デフォルトモードは `magic`** — Nanoは `default_mode = "magic"` がプリセットされており、ext4イメージを管理するデーモンがない構成ではバインドマウントを優先します。
- **モジュールモードマーカー** — インストール時の音量キー選択で、管理対象モジュールのルートに空の `overlay` または `magic` マーカーを置き、Nano は白リストではなくそれを読み取ります。マーカー名は大文字小文字を区別せずに扱われます。
- **常駐するHybrid Mountプロセスなし** — 起動時のマウント完了後、Nanoバイナリは終了します。

予測可能でデーモンフリーのマウント統合を行い、常駐コンポーネントを減らしたい場合にNanoを選んでください。

### 機能マトリックス

| 機能 | Lite | Nano |
|------|------|------|
| OverlayFS バックエンド | あり | マーカー方式 |
| Magic Mount バックエンド | あり | あり（デフォルト） |
| WebUI | あり | なし |
| CLI（`hybrid-mount` サブコマンド） | あり | なし |
| デーモン（Unix + TCP/SSE） | あり | なし |
| 設定キャッシュとランタイム適用 | あり | なし |
| Cargo features | `control-plane` のみ | なし |
| ZIPサイズ（約） | ~2 MB | ~1 MB |

## 特徴

- **決定論的プランニング** — 競合は計画段階で検出され、起動時にランダムに発見されることはありません。
- **内蔵 WebUI** — モジュール管理、設定編集、ランタイム状態の監視。
- **設定キャッシュ** — 増分パッチと即時適用をサポートするランタイム設定キャッシュ。
- **リカバリーフレンドリー** — 古いランタイムファイルは自動クリーンアップ。設定ミスは `api config-reset` でリセット可能。
- **自動化フレンドリー** — スクリプトや外部コントローラー向けのJSON-over-Unix-socketデーモンプロトコル + HTTP API。

---

## クイックスタート

### インストール

1. デバイスに [KernelSU](https://kernelsu.org/) または [APatch](https://apatch.dev/) をインストールします。
2. [GitHub Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases) から最新の Hybrid Mount `lite` または `nano` リリースZIPをダウンロードします。
3. RootマネージャーのモジュールインストーラーからZIPをフラッシュします。
4. 再起動します。Hybrid Mount が自動的に環境を検出し、デフォルトのoverlayポリシーを適用します。

### インストール後

```bash
# ランタイム状態の確認
hybrid-mount daemon status

# 検出されたモジュールの一覧表示
hybrid-mount api modules-list
```

WebUIにアクセスするには（Liteフレーバー）、Rootマネージャーアプリ（KernelSUまたはAPatch）を開き、モジュール一覧からHybrid Mountを見つけてタップします — マネージャーが内蔵WebViewでWebUIを起動します。

### モジュールのマウントモード変更

```toml
# /data/adb/hybrid-mount/config.toml
[rules.my_module]
default_mode = "magic"

[rules.my_module.paths]
"system/bin/problematic_binary" = "ignore"
```

---

## マウントモード

| モード | バックエンド | 最適な用途 |
|------|------------|----------|
| `overlay` | OverlayFS | 競合なくファイルを追加・置換するモジュール。デフォルトモード。 |
| `magic` | Bind mount | ファイルごとの直接置換が必要なモジュール。 |
| `ignore` | — | 特定のパスをマウント処理から除外。 |

### OverlayFS ストレージモード

OverlayFSバックエンドは、upper/workレイヤーに2つのストレージ戦略をサポートしています：

- `ext4`（デフォルト）— ext4ディスクイメージを作成。再起動後も永続化され、xattrをサポート。
- `tmpfs` — tmpfsマウントを使用。揮発性で軽量だが、再起動時に失われます。

```toml
overlay_mode = "ext4"
```

---

## WebUI

Hybrid Mount は、デーモンがローカルTCPソケット（HTTP/SSE）で提供する **SolidJSベースのWebUI** を内蔵しています。CLIおよび自動化クライアントはUnixソケット経由で通信します。デーモンは起動時にWebUIアクセスURLをlogcatに出力します。

WebUIは、**Rootマネージャーアプリ**（KernelSUまたはAPatchマネージャー）から直接開くように設計されています — モジュールエントリをタップすると、マネージャーが内蔵WebViewでWebUIを起動します。デバイス上で外部ブラウザは不要です。

### 機能

- **ステータスダッシュボード** — ライブマウント統計、アクティブパーティション、ストレージモード、デーモンヘルス。
- **モジュール管理** — 検出された全モジュールとその有効なマウントモードの一覧表示。インタラクティブなモード変更。
- **設定エディター** — バリデーション付きの完全な config.toml 編集。モジュールごとのパスルール設定を含む。

### 対応言語

WebUI は次のロケールを提供しています：

- English (`en-US`, デフォルト)
- Español (`es-ES`)
- Italiano (`it-IT`)
- 日本語 (`ja-JP`)
- Русский (`ru-RU`)
- Українська (`uk-UA`)
- Tiếng Việt (`vi-VN`)
- 简体中文 (`zh-CN`)
- 繁體中文 (`zh-TW`)

README 文書は [English](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/README.md)、[简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH.md)、[繁體中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH_TW.md)、[日本語](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_JP.md)、[Español](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ES.md)、[Italiano](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_IT.md)、[Русский](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_RU.md)、[Українська](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_UK.md)、[Tiếng Việt](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_VI.md) で提供されています。

### アクセス方法

WebUIは `http://127.0.0.1:<ランダムポート>` で暗号化アクセストークンを使用して実行されます。デーモンがライフサイクルを管理し、別途Webサーバーは不要です。デバイス上ではRootマネージャーのWebViewから開き、リモートではADBポート転送を使用します。

---

## 設定

デフォルトパス：`/data/adb/hybrid-mount/config.toml`

### トップレベルフィールド

| キー | 型 | デフォルト | 説明 |
| --- | --- | --- | --- |
| `moduledir` | string | `/data/adb/modules` | モジュールディレクトリ。 |
| `mountsource` | string | 自動検出 | ランタイムソースタグ（`KSU`、`APatch`）。 |
| `overlay_mode` | `ext4` \| `tmpfs` | `ext4` | Overlay upper/work ストレージモード。 |
| `disable_umount` | bool | `false` | umount操作をスキップ（デバッグ専用）。 |
| `default_mode` | `overlay` \| `magic` | `overlay` | グローバルデフォルトマウントポリシー。 |
| `daemon_startup_mode` | `on-demand` \| `persistent` | `on-demand` | デーモン起動動作。 |
| `rules` | map | `{}` | モジュール・パス単位のマウントポリシー。 |

### 例

```toml
moduledir = "/data/adb/modules"
overlay_mode = "ext4"
default_mode = "overlay"
daemon_startup_mode = "on-demand"

[rules.viper4android]
default_mode = "magic"

[rules.viper4android.paths]
"system/etc/audio_policy.conf" = "overlay"

```

---


## ポリシーリファレンス

### 優先順位

複数のポリシーが1つのパスに適用可能な場合、評価順序は次のとおりです：

1. **パスレベルオーバーライド** — `rules.<module>.paths["<path>"]`
2. **モジュールレベルデフォルト** — `rules.<module>.default_mode`
3. **グローバルデフォルト** — `default_mode`

### 動作マトリックス

| ルール結果 | バックエンド利用可？ | 実際の動作 |
| --- | --- | --- |
| `overlay` | はい | OverlayFS でマウント。 |
| `overlay` | いいえ | スキップして失敗として報告。 |
| `magic` | n/a | Magic Mount でマウント。 |
| `ignore` | n/a | マウントしない。 |

### モジュールマーカーファイル

Hybrid Mount は、モジュールディレクトリ内のマーカーファイルも認識します。これらのマーカーは通常のファイルとして置くことを想定しており、判定にはファイル名のみを使用します。マーカー名はASCII英字について大文字小文字を区別せずに照合されるため、`DISABLE`、`Disable`、`disable` は同じマーカーとして扱われます。

| マーカー | 場所 | 効果 |
| --- | --- | --- |
| `disable` | モジュールルート | モジュールをマウント計画から除外し、無効として表示します。 |
| `remove` | モジュールルート | モジュールをマウント計画から除外します。通常はRootマネージャーが削除時に作成します。 |
| `skip_mount` | モジュールルート | モジュールのマウント処理をスキップし、ランタイムのskipリストに記録します。 |
| `mount_error` | モジュールルート | マウント失敗後にスキップされたモジュールを示します。リカバリー処理やデーモンコマンドが作成または削除する場合があります。 |
| `overlay` / `magic` | モジュールルート、Nanoビルド | Nanoビルドでモジュールのデフォルトマウントバックエンドを選択します。Liteビルドでは設定ルールを使用します。 |
| `.replace` | モジュールディレクトリ内 | そのディレクトリに置換セマンティクスを適用します。マーカー自体は通常のモジュール内容としてコピーされません。準備済みのOverlayFSレイヤーではディレクトリを保持し、対応環境ではoverlay opaqueメタデータを設定します。 |

同じディレクトリに同一マーカーの大文字小文字違いが複数ある場合、クリーンアップ処理は一致するすべての変種を削除します。

### 実用的なレシピ

- **1つの問題バイナリをbind mount、残りをoverlayに**：モジュールのデフォルトを `overlay` に設定し、そのバイナリパスを `magic` で上書き。
- **競合ファイルを一時的に除外**：パスを `ignore` に設定。

---

## CLI

```bash
hybrid-mount [OPTIONS] [COMMAND]
```

### グローバルオプション

| フラグ | 説明 |
| ---- | ---- |
| `-c, --config <PATH>` | カスタム設定ファイルのパス。 |

### サブコマンド

| コマンド | 説明 |
| ------- | ---- |
| `gen-config` | デフォルト設定ファイルを生成。 |
| `logs` | 最近のデーモンログを表示。 |
| `api storage` | ストレージモードを照会（ext4/tmpfs）。 |
| `api mount-stats` | マウント統計を表示。 |
| `api mount-topology` | マウントトポロジツリーを表示。 |
| `api partitions` | 管理パーティションを一覧表示。 |
| `api system-info` | システム情報を表示。 |
| `api version` | デーモンバージョンを表示。 |
| `api config-get` | 有効な設定をJSONで表示。 |
| `api config-set --config <JSON>` | 設定全体を置換。 |
| `api config-patch --patch <JSON>` | パッチを設定にマージ。 |
| `api config-reset` | 設定をデフォルトにリセット。 |
| `api modules-list` | 検出されたモジュールを一覧表示。 |
| `api modules-apply --modules <JSON>` | モジュールモード変更を適用。 |
| `api features` | サポート機能を一覧表示。 |
| `api kernel-uname` | カーネルunameを表示。 |
| `api open-url --url <URL>` | デバイスでURLを開く。 |
| `api reboot` | デバイスを再起動。 |
| `daemon launch` | デーモンをフォアグラウンドで起動。 |
| `daemon serve` | デーモンを起動（サービスモード）。 |
| `daemon ping` | デーモンの生存確認。 |
| `daemon webui-start` | WebUIのみ起動。 |
| `daemon stop` | デーモンを停止。 |
| `daemon status` | デーモンのランタイム状態を照会。 |

---

## アーキテクチャ

```text
┌─────────────────────────────────────────────┐
│                  config.toml                  │
└──────────────────┬──────────────────────────┘
                   ▼
┌─────────────────────────────────────────────┐
│              インベントリ検出                   │
│         モジュールツリーを走査、エントリを分類      │
└──────────────────┬──────────────────────────┘
                   ▼
┌─────────────────────────────────────────────┐
│              マウントプランナー                  │
│     ルールを評価 (パス > モジュール > グローバル)   │
│     overlay / magic 計画を生成                │
└──────────────────┬──────────────────────────┘
                   ▼
┌─────────────────────────────────────────────┐
│              エグゼキューター                    │
│  ┌──────────┐ ┌──────────┐ │
│  │ OverlayFS│ │  Magic   │ │
│  │ 実行器   │ │  Mount   │ │
│  └──────────┘ └──────────┘ │
└──────────────────┬──────────────────────────┘
                   ▼
┌─────────────────────────────────────────────┐
│            ランタイム状態 + デーモン              │
│     状態の永続化 → Unixソケット → WebUI/CLI      │
└─────────────────────────────────────────────┘
```

エグゼキューターは **typestate ステートマシン**（`src/core/controller.rs`）によって駆動されます：`MountController<Init> → StorageReady → Planned → Executed`。各遷移は1つのパイプラインステージを表し、マウントプロセスが常に明確に定義された状態にあることを保証します。

### ソース構成

```text
src/
├── conf/          設定スキーマ、TOMLローダー、CLI定義とハンドラー
├── domain/        コア型：MountMode、ModuleRules、パスマッチング
├── partitions/    管理パーティションの自動検出
├── core/
│   ├── inventory/ モジュール検出と一覧表示
│   ├── ops/       マウント計画生成とバックエンド別実行
│   ├── daemon/    Unix + TCP デュアルプロトコルデーモン（CLI + WebUI/SSE）
│   ├── api/       WebUIエンドポイント向けペイロードビルダー
│   ├── startup/   起動シーケンス、リカバリー、リトライロジック
│   ├── storage/   共有ストレージヘルパー（ext4イメージ、tmpfs）
│   └── runtime_state/ デーモン状態の永続化
├── mount/
│   ├── overlayfs/ OverlayFS バックエンド（ext4イメージ / tmpfs）
│   ├── magic_mount/ Bind mount バックエンド
├── sys/           低レベル：マウントシステムコール
└── utils/         ログ、パスユーティリティ、検証

webui/
├── src/
│   ├── routes/    ページコンポーネント（状態、設定、モジュール、情報）
│   ├── components/ 共有UIコンポーネント（ナビバー、トースト、スケルトン）
│   ├── lib/       APIブリッジ、ストア、コーデック、i18n
│   └── locales/   9言語の国際化対応

xtask/             ビルドとリリースの自動化
module/            モジュールパッケージスクリプトと静的アセット
```

---

## ビルド

### 前提条件

- Rust nightly（`rust-toolchain.toml` 参照）
- Android NDK r27+ および `cargo-ndk`
- Node.js 20+ と pnpm（WebUI用）

### コマンド

```bash

# Liteリリースパッケージ（バイナリ + WebUI） → output/
cargo run -p xtask -- build --release --flavor lite

# Nanoリリースパッケージ（設定専用、WebUI/CLI/デーモンなし） → output/
cargo run -p xtask -- build --release --flavor nano

# バイナリのみ（WebUIをスキップ）
cargo run -p xtask -- build --release --skip-webui

# ローカル arm64 デバッグビルド
./scripts/build-local.sh

# ローカル lite デバッグビルド
./scripts/build-local.sh --lite

# ローカル nano デバッグビルド
./scripts/build-local.sh --nano


# WebUI開発サーバー（ホットリロード）
cd webui && pnpm install && pnpm dev

# リント
cargo run -p xtask -- lint
cd webui && pnpm lint

# テスト実行
cargo +nightly test
cd webui && pnpm test
```

### リリースプロファイル

リリースプロファイルは、バイナリサイズを抑えるために `opt-level = 3`、`lto = "fat"`、`codegen-units = 1`、`strip = true`、`panic = "abort"` を使用します。

### CIゲートとフィーチャーフラグのリント

すべての変更は以下のCIチェックに合格する必要があります（`.github/workflows/` で定義）：

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`（警告はエラーとして扱われます）
- `cargo test --all-targets --workspace`
- WebUI: `pnpm lint` + `pnpm test`
- 全ソースファイルのライセンスヘッダーチェック

変更を加える際は、**lite**（`--no-default-features --features control-plane`）と **nano**（`--no-default-features`）のフレーバー組み合わせがコンパイルできることを確認してください。daemon/CLI/WebUI API に関するコードは `#[cfg(feature = "control-plane")]` の背後に配置する必要があります。

---

## 運用上の注意

- **マウントソースの自動検出**：新規インストールでは実行環境を自動検出します。自動検出が失敗した場合のみ `mountsource` を明示的に設定してください。
- **不良設定からの回復**：`hybrid-mount api config-reset` を実行してデフォルトにリセットし、ルールを段階的に再適用します。`gen-config` で新しい設定ファイルを再生成することもできます。
- **設定キャッシュ**：ランタイムは設定キャッシュを維持します。変更を即時適用するには `api config-patch --apply-runtime` を使用するか、デーモンを再起動します。
- **バイナリサイズ**：大規模なリファクタリングよりも、依存機能のトリミングとプロファイルチューニングを優先してください。

---

## ライセンス

[GPL-3.0](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/LICENSE) の下でライセンスされています。
