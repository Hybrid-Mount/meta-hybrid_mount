# Hybrid Mount

<img src="https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/main/icon.svg" align="right" width="120" />

![Language](https://img.shields.io/badge/Language-Rust-orange?style=flat-square&logo=rust)
![Platform](https://img.shields.io/badge/Platform-Android-green?style=flat-square&logo=android)
![License](https://img.shields.io/badge/License-GPL--3.0-blue?style=flat-square)
![Version](https://img.shields.io/github/v/tag/Hybrid-Mount/meta-hybrid_mount?label=Version&color=8A2BE2&style=flat-square)

Hybrid Mount adalah metamodul orkestrasi mount untuk **KernelSU** dan **APatch**.
Modul ini menggabungkan file-file modul ke dalam partisi Android melalui mesin kebijakan terpadu yang didukung oleh dua backend mount:

- **OverlayFS** — mount berlapis dengan penyimpanan upper/work.
- **Magic Mount** — bind mount untuk penggantian jalur secara langsung.

**WebUI SolidJS** bawaan menyediakan manajemen grafis, pemantauan status secara langsung, dan pengeditan konfigurasi.

Rilis dipublikasikan dalam dua varian — lihat [Varian Build](#build-flavors) untuk perbandingan mendetail. Kecuali disebutkan lain, bagian selanjutnya dari README ini menjelaskan build Lite bawaan.

**[English](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/README.md)** &nbsp; **[简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH.md)** &nbsp; **[繁體中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH_TW.md)** &nbsp; **[日本語](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_JP.md)** &nbsp; **[Español](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ES.md)** &nbsp; **[Italiano](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_IT.md)** &nbsp; **[Русский](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_RU.md)** &nbsp; **[Українська](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_UK.md)** &nbsp; **[Tiếng Việt](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_VI.md)** &nbsp; **[Bahasa Indonesia](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ID.md)** &nbsp; **[Türkçe](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_TR.md)**

---

## Daftar Isi

- [Fitur](#features)
- [Varian Build](#build-flavors)
- [Memulai Cepat](#quick-start)
- [Mode Mount](#mount-modes)
- [WebUI](#webui)
- [Dukungan Bahasa](#language-support)
- [Konfigurasi](#configuration)
- [Referensi Kebijakan](#policy-reference)
- [CLI](#cli)
- [Arsitektur](#architecture)
- [Build](#build)
- [Lisensi](#license)

---

## Varian Build

Hybrid Mount dirilis dalam dua varian, masing-masing menargetkan kasus penggunaan yang berbeda:

| Varian | Biner | WebUI | Daemon / CLI | Kasus penggunaan |
|--------|--------|-------|-------------|----------|
| **Lite** | Ya | Ya | Ya | Rilis bawaan: WebUI, daemon, CLI, serta backend OverlayFS dan Magic Mount. |
| **Nano** | Ya | Tidak | Tidak | Untuk minimalis yang hanya menginginkan orkestrasi mount melalui file config — tanpa daemon runtime, tanpa WebUI, tanpa CLI. |

### Lite

Lite adalah rilis bawaan. Varian ini mencakup WebUI SolidJS, daemon Unix-socket dengan HTTP/SSE, CLI, serta kedua backend OverlayFS dan Magic Mount:

- Anda menginginkan WebUI dan mesin kebijakan lengkap.
- Anda menginginkan unduhan yang lebih kecil dengan tetap mempertahankan WebUI dan antarmuka manajemen daemon.

Build Lite hanya menggunakan set fitur `control-plane` (`--no-default-features --features control-plane`).

### Nano

Varian `nano` adalah build **khusus konfigurasi** (`--no-default-features` — tidak ada fitur Cargo yang diaktifkan). Varian ini menghilangkan WebUI, daemon, CLI, dan seluruh infrastruktur control-plane. Yang tersisa adalah biner minimal yang membaca `config.toml`, membuat rencana mount, lalu mengeksekusinya — kemudian keluar. Karakteristik utama:

- **Tanpa daemon runtime** — tanpa proses latar belakang, tanpa socket, tanpa WebUI, tanpa subperintah CLI.
- **Tanpa WebUI** — aset `webroot/` dan `launcher.png` dihapus dari paket.
- **Operasi khusus mount** — biner berjalan saat boot, me-mount semuanya sesuai konfigurasi, lalu berhenti.
- **Mode bawaan adalah `magic`** — Nano dikirim dengan `default_mode = "magic"` yang telah diatur di konfigurasinya, lebih memilih bind mount ketika tidak ada daemon yang tersedia untuk mengelola image ext4.
- **Penanda mode modul** — pemilihan tombol volume saat instalasi menulis penanda kosong `overlay` atau `magic` di root setiap modul yang dikelola, dan Nano membaca penanda tersebut alih-alih whitelist. Nama file penanda harus menggunakan ejaan huruf kecil yang persis.
- **Tanpa proses Hybrid Mount yang menetap** — setelah proses mount saat boot selesai, biner Nano langsung keluar.

Pilih Nano jika Anda menginginkan orkestrasi mount yang dapat diprediksi, bebas daemon, dengan permukaan runtime yang lebih kecil.

### Matriks fitur

| Fitur | Lite | Nano |
|---------|------|------|
| Backend OverlayFS | Ya | Berbasis penanda |
| Backend Magic Mount | Ya | Ya (bawaan) |
| WebUI | Ya | Tidak |
| CLI (subperintah `hybrid-mount`) | Ya | Tidak |
| Daemon (Unix + TCP/SSE) | Ya | Tidak |
| Penerapan konfigurasi runtime | Tidak (tersimpan untuk boot berikutnya) | Tidak |
| Fitur Cargo | `control-plane` only | none |
| Ukuran ZIP (perkiraan) | ~2 MB | ~1 MB |

## Fitur

- **Dua backend, satu mesin kebijakan** — tetapkan jalur ke OverlayFS atau Magic Mount dengan granularitas per jalur.
- **Perencanaan deterministik** — konflik terdeteksi pada saat perencanaan, bukan ditemukan secara acak saat boot.
- **WebUI bawaan** — kelola modul, edit konfigurasi, dan pantau status runtime.
- **Pembaruan konfigurasi runtime** — patch konfigurasi yang tervalidasi disimpan dan berlaku pada boot berikutnya. WebUI melaporkan hal ini secara eksplisit alih-alih berpura-pura perubahan diterapkan langsung.
- **Pelaporan kegagalan eksplisit** — status tidak valid dan kesalahan konfigurasi segera dimunculkan; reset konfigurasi adalah aksi eksplisit `api config-reset`.
- **Ramah otomatisasi** — protokol daemon JSON-over-Unix-socket + HTTP API untuk skrip atau pengendali eksternal.

---

## Memulai Cepat

### Instalasi

1. Pasang [KernelSU](https://kernelsu.org/) atau [APatch](https://apatch.dev/) di perangkat Anda.
2. Unduh ZIP rilis Hybrid Mount Lite atau Nano terbaru dari [GitHub Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases).
3. Flash ZIP melalui penginstal modul pada root manager Anda.
4. Pada instalasi baru, pilih mode mount bawaan: Tombol Volume Atas memilih OverlayFS, Tombol Volume Bawah memilih Magic Mount, dan waktu tunggu 10 detik memilih OverlayFS. Ini adalah satu-satunya prompt dari penginstal. Nano melewati langkah pengaturan ini.
5. Reboot. Hybrid Mount akan mendeteksi lingkungan Anda secara otomatis dan menerapkan kebijakan bawaan yang dipilih.

### Pasca-instalasi

```bash
# Check runtime status
hybrid-mount daemon status

# List detected modules
hybrid-mount api modules-list
```

Untuk mengakses WebUI, buka aplikasi root manager Anda (KernelSU atau APatch), cari Hybrid Mount di daftar modul, lalu ketuk entri tersebut — root manager akan membuka WebUI di WebView tertanam.

### Mengubah mode mount untuk sebuah modul

```toml
# /data/adb/hybrid-mount/config.toml
[rules.my_module]
default_mode = "magic"

[rules.my_module.paths]
"system/bin/problematic_binary" = "ignore"
```

### Bind mount kustom

```toml
[[custom_mounts]]
source = "/data/local/tmp/replacement.conf"
target = "/system/etc/replacement.conf"
```

Bind mount kustom dijalankan setelah eksekusi OverlayFS/Magic Mount modul. Sumber dan target harus sama-sama ada dan harus sama-sama berupa file atau sama-sama berupa direktori.

---

## Mode Mount

| Mode | Backend | Paling cocok untuk |
|------|---------|----------|
| `overlay` | OverlayFS | Modul yang menambah atau mengganti file tanpa konflik. Mode bawaan. |
| `magic` | Bind mount | Modul yang memerlukan penggantian per file secara langsung. |
| `ignore` | — | Mengecualikan jalur tertentu dari pemrosesan mount apa pun. |

### Mode penyimpanan OverlayFS

Backend OverlayFS mendukung dua strategi penyimpanan untuk lapisan upper/work:

- `ext4` (bawaan) — membuat image staging ext4 baru untuk setiap proses mount. Mendukung overlay xattr; image dihapus setelah mount difinalisasi.
- `tmpfs` — menggunakan mount tmpfs. Volatile, lebih ringan, tetapi hilang saat reboot.

```toml
overlay_mode = "ext4"
```

---

## WebUI

Hybrid Mount menyertakan **WebUI berbasis SolidJS** yang disajikan oleh daemon melalui socket TCP lokal (HTTP/SSE). Klien CLI dan otomatisasi berkomunikasi melalui socket Unix. Daemon mencetak URL akses WebUI ke logcat saat startup.

Buka WebUI langsung dari **aplikasi root manager** Anda (manajer KernelSU atau APatch) — ketuk entri modul dan manajer akan membukanya di WebView tertanam. Tidak diperlukan browser eksternal di perangkat.

### Kemampuan

- **Dasbor status** — statistik mount langsung, partisi aktif, mode penyimpanan, kesehatan daemon.
- **Manajemen modul** — tampilkan semua modul yang terdeteksi beserta mode mount efektifnya; terapkan perubahan mode secara interaktif.
- **Editor konfigurasi** — pengeditan config.toml lengkap dengan validasi, termasuk aturan jalur per modul.

### Dukungan Bahasa

WebUI saat ini dikirim dengan locale berikut:

- English (`en-US`, default)
- Español (`es-ES`)
- Italiano (`it-IT`)
- 日本語 (`ja-JP`)
- Русский (`ru-RU`)
- Українська (`uk-UA`)
- Tiếng Việt (`vi-VN`)
- Bahasa Indonesia (`id-ID`)
- Türkçe (`tr-TR`)
- 简体中文 (`zh-CN`)
- 繁體中文 (`zh-TW`)

Dokumentasi README tersedia dalam [English](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/README.md), [Simplified Chinese](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH.md), [Traditional Chinese](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH_TW.md), [Japanese](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_JP.md), [Spanish](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ES.md), [Italian](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_IT.md), [Russian](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_RU.md), [Ukrainian](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_UK.md), [Vietnamese](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_VI.md), [Bahasa Indonesia](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ID.md), dan [Turkish](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_TR.md).

### Akses

WebUI berjalan di `http://127.0.0.1:<random-port>` dengan token akses kriptografis. Daemon mengelola siklus hidupnya — tidak diperlukan server web terpisah. Di perangkat, buka melalui WebView root manager Anda; dari jarak jauh, teruskan port-nya melalui ADB.

---

## Konfigurasi

Jalur bawaan: `/data/adb/hybrid-mount/config.toml`.

### Field tingkat atas

| Key | Tipe | Bawaan | Deskripsi |
| --- | --- | --- | --- |
| `moduledir` | string | `/data/adb/modules` | Direktori sumber modul. |
| `mountsource` | string | auto-detect | Tag sumber runtime (`KSU`, `APatch`). |
| `overlay_mode` | `ext4` \| `tmpfs` | `ext4` | Mode penyimpanan upper/work overlay. |
| `disable_umount` | bool | `false` | Lewati operasi umount (khusus debug). |
| `default_mode` | `overlay` \| `magic` | `overlay` | Kebijakan mount bawaan global. |
| `rules` | map | `{}` | Kebijakan mount per modul dan per jalur. |

### Contoh

```toml
moduledir = "/data/adb/modules"
overlay_mode = "ext4"
default_mode = "overlay"

[rules.viper4android]
default_mode = "magic"

[rules.viper4android.paths]
"system/etc/audio_policy.conf" = "overlay"
```

---


## Referensi Kebijakan

### Prioritas

Ketika beberapa kebijakan dapat berlaku untuk suatu jalur, urutan evaluasinya adalah:

1. **Override tingkat jalur** — `rules.<module>.paths["<path>"]`
2. **Bawaan tingkat modul** — `rules.<module>.default_mode`
3. **Bawaan global** — `default_mode`

### Matriks perilaku

| Hasil aturan | Backend tersedia? | Perilaku efektif |
| --- | --- | --- |
| `overlay` | Ya | Mount dengan OverlayFS. |
| `overlay` | Tidak | Lewati dan laporkan sebagai gagal. |
| `magic` | n/a | Mount dengan Magic Mount. |
| `ignore` | n/a | Tidak melakukan mount. |

### File penanda modul

Hybrid Mount juga mengenali file penanda di direktori modul. Penanda ini diharapkan berupa file biasa; hanya nama filenya yang digunakan. Nama file penanda bersifat ketat dan peka huruf besar/kecil: gunakan nama persis seperti yang tercantum di bawah.

| Penanda | Lokasi | Efek |
| --- | --- | --- |
| `disable` | Root modul | Mengecualikan modul dari perencanaan mount dan melaporkannya sebagai nonaktif. |
| `remove` | Root modul | Mengecualikan modul dari perencanaan mount; biasanya dibuat oleh root manager selama penghapusan. |
| `skip_mount` | Root modul | Mengecualikan modul dari pemrosesan mount dan mencatatnya di daftar skip runtime. |
| `overlay` / `magic` | Root modul, build Nano | Memilih backend mount bawaan modul untuk build Nano. Build Lite menggunakan aturan konfigurasi. |
| `.replace` | Di dalam direktori modul | Menerapkan semantik penggantian pada direktori yang memuatnya. Penanda itu sendiri tidak disalin sebagai konten modul normal; lapisan overlay yang disiapkan mempertahankan direktori tersebut dan menetapkan metadata opaque overlay jika didukung. |

### Resep praktis

- **Satu biner bermasalah di bind mount, sisanya di overlay**: atur bawaan modul ke `overlay`, timpa jalur biner tersebut ke `magic`.
- **Mengecualikan sementara file yang berkonflik**: atur jalur tersebut ke `ignore`.

---

## CLI

```bash
hybrid-mount [OPTIONS] [COMMAND]
```

### Opsi global

| Flag | Deskripsi |
| ---- | ----------- |
| `-c, --config <PATH>` | Jalur file konfigurasi kustom. |

### Subperintah

| Perintah | Deskripsi |
| ------- | ----------- |
| `gen-config` | Buat file konfigurasi bawaan. |
| `logs` | Cetak log daemon terbaru. |
| `api storage` | Tanyakan mode penyimpanan (ext4/tmpfs). |
| `api mount-stats` | Cetak statistik mount. |
| `api mount-topology` | Cetak pohon topologi mount. |
| `api partitions` | Tampilkan partisi yang dikelola. |
| `api system-info` | Cetak informasi sistem. |
| `api version` | Cetak versi daemon. |
| `api config-get` | Cetak konfigurasi efektif sebagai JSON. |
| `api config-set --config <JSON>` | Ganti seluruh konfigurasi (berlaku pada boot berikutnya). |
| `api config-patch --patch <JSON>` | Gabungkan patch ke konfigurasi (berlaku pada boot berikutnya; `--apply-runtime` adalah no-op yang sudah usang). |
| `api config-reset` | Reset konfigurasi ke bawaan. |
| `api modules-list` | Tampilkan modul yang terdeteksi. |
| `api modules-apply --modules <JSON>` | Terapkan perubahan mode modul. |
| `api features` | Tampilkan fitur yang didukung. |
| `api kernel-uname` | Cetak uname kernel. |
| `api open-url --url <URL>` | Buka URL di perangkat. |
| `api reboot` | Reboot perangkat. |
| `daemon launch` | Jalankan daemon di latar depan. |
| `daemon serve` | Jalankan daemon (mode layanan). |
| `daemon ping` | Periksa status hidup daemon. |
| `daemon webui-start` | Jalankan WebUI saja. |
| `daemon stop` | Hentikan daemon. |
| `daemon status` | Tanyakan status runtime daemon. |

### Pencatatan dan latensi

Proses normal menggunakan level `info`. Atur `HYBRID_MOUNT_LOG_LEVEL` ke `off`, `error`, `warn`, `info`, `debug`, atau `trace` sebelum meluncurkan proses untuk mengubah verbositas. `debug` mengembalikan detail per modul dan per mount.

Tahapan boot berbutir kasar menghasilkan catatan terstruktur seperti:

```text
[latency] scope=executor, stage=overlay_apply, status=ok, elapsed_us=1842
```

Pengukuran menggunakan jam monotonik dan tidak mengukur waktu per file. Tahap yang berakhir dengan error dicatat dengan `status=aborted`.

---

## Arsitektur

```text
┌─────────────────────────────────────────────┐
│                  config.toml                  │
└──────────────────┬──────────────────────────┘
                   ▼
┌─────────────────────────────────────────────┐
│              Inventory Discovery              │
│         Scan module tree, classify entries    │
└──────────────────┬──────────────────────────┘
                   ▼
┌─────────────────────────────────────────────┐
│              Mount Planner                    │
│    Evaluate rules (path > module > global)    │
│    Generate overlay / magic plan              │
└──────────────────┬──────────────────────────┘
                   ▼
┌─────────────────────────────────────────────┐
│              Executors                        │
│  ┌──────────┐ ┌──────────┐                  │
│  │ OverlayFS│ │  Magic   │                  │
│  │ executor │ │  Mount   │                  │
│  └──────────┘ └──────────┘                  │
└──────────────────┬──────────────────────────┘
                   ▼
┌─────────────────────────────────────────────┐
│            Runtime State + Daemon             │
│   Persist state → Unix socket → WebUI/CLI     │
└─────────────────────────────────────────────┘
```

Eksekutor digerakkan oleh **state machine typestate** (`src/core/controller.rs`): `MountController<Init> → StorageReady → Planned → Executed`. Setiap transisi merepresentasikan satu tahap pipeline, memastikan proses mount selalu berada dalam status yang terdefinisi dengan baik.

### Tata letak sumber

```text
src/
├── conf/          Config schema, TOML loader, CLI definition, handlers
├── domain/        Core types: MountMode, ModuleRules, path matching
├── partitions/    Managed partition auto-discovery
├── core/
│   ├── inventory/ Module discovery and listing
│   ├── ops/       Mount plan generation and per-backend execution
│   ├── daemon/    Unix + TCP dual-protocol daemon (CLI + WebUI/SSE)
│   ├── api/       Payload builders for WebUI endpoints
│   ├── startup/   Boot sequence and daemon handoff
│   ├── storage/   Shared storage helpers (ext4 image, tmpfs)
│   └── runtime_state/ Daemon state persistence
├── mount/
│   ├── overlayfs/ OverlayFS backend (ext4 image / tmpfs)
│   ├── magic_mount/ Bind-mount backend
│   └── custom_bind/ Custom bind mounts
├── sys/           Low-level: mount syscalls and filesystem helpers
└── utils/         Logging, path utilities, validation

webui/
├── src/
│   ├── routes/    Page components (Status, Config, Modules, Info)
│   ├── components/ Shared UI components (NavBar, Toast, Skeleton)
│   ├── lib/       API bridge, stores, codecs, i18n
│   └── locales/   11-language internationalization

xtask/             Build and release automation
module/            Module packaging scripts and static assets
```

---

## Build

### Prasyarat

- Rust nightly (dari `rust-toolchain.toml`)
- Android NDK r27+ dan `cargo-ndk`
- Node.js 20.19+ atau 22.12+, dan pnpm 10.34.5 (untuk WebUI)

### Perintah

```bash
# Lite release package (binary + WebUI) → output/
cargo run -p xtask -- build --release --flavor lite

# Nano release package (config-only, no WebUI/CLI/daemon) → output/
cargo run -p xtask -- build --release --flavor nano

# Binary only (skip WebUI)
cargo run -p xtask -- build --release --skip-webui

# Local arm64 debug build
./scripts/build-local.sh

# Local nano debug build
./scripts/build-local.sh --nano

# WebUI dev server (hot reload)
cd webui && pnpm install && pnpm dev

# Full local verification (Rust fmt/clippy/tests + WebUI lint/test)
cargo run -p xtask -- lint

# Focused test runs
cargo +nightly test
cd webui && pnpm test
```

### Profil rilis

Profil rilis menggunakan `opt-level = "z"`, `lto = "fat"`, `codegen-units = 1`, `strip = true`, dan `panic = "abort"` untuk mengurangi ukuran biner.

### Gerbang CI dan linting feature flag

Setiap perubahan harus lolos pemeriksaan CI berikut (didefinisikan di `.github/workflows/`):

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings` (warning dianggap error)
- `cargo test --all-targets --workspace`
- `cargo test -p hybrid-mount --no-default-features --features control-plane --lib`
- `cargo test -p hybrid-mount --no-default-features --lib`
- WebUI: `pnpm lint` + `pnpm test`
- Pemeriksaan header lisensi pada semua file sumber

`xtask lint` mencerminkan gerbang CI lokal: Rust fmt, clippy/test, pengujian **lite** (`--no-default-features --features control-plane`), pengujian **nano** (`--no-default-features`), serta lint/pengujian WebUI. Kode yang menyentuh API daemon/CLI/WebUI harus berada di balik `#[cfg(feature = "control-plane")]`.

---

## Dukungan late-load (jailbreak)

KernelSU dapat dimuat setelah boot dalam mode late-load (skenario jailbreak / bootloader terkunci). Hybrid Mount mendukung mode ini:

- Instalasi tidak lagi dibatalkan ketika `KSU_LATE_LOAD=1`.
- KernelSU menjalankan `module/emulated-soft-reboot.sh` selama soft reboot emulasinya; skrip tersebut memanggil `hybrid-mount emulated-soft-reboot` untuk melepas mount dari proses sebelumnya sebelum modul di-mount lagi, sehingga mount tidak pernah menumpuk.
- Pembersihan yang sama juga dijalankan saat startup ketika `KSU_LATE_LOAD=1`.

Pembersihan hanya melepas mount yang dapat dikaitkan secara tepat dengan Hybrid Mount: mount overlay yang opsi-opsinya merujuk ke workspace atau root data miliknya, workspace yang persis cocok dengan `/mnt/hm_<10 chars>` atau `/debug_ramdisk/hm_<10 chars>`, serta target Magic/kustom persis yang disimpan oleh proses sebelumnya (ditambah target kustom yang dikonfigurasi saat ini). Pembersihan tidak pernah memercayai sumber mount bersama KernelSU/APatch sebagai bukti kepemilikan.

---

## Lisensi

Dilisensikan di bawah [GPL-3.0](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/LICENSE).
