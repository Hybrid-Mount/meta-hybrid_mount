# Hybrid Mount

Hybrid Mount adalah metamodul mount hibrida untuk KernelSU dan APatch. Saat boot, modul ini memindai modul lain lalu memilih OverlayFS, Magic Mount, atau abaikan untuk setiap entri berdasarkan aturan global, modul, dan jalur. Direktori sumber modul selalu diperlakukan sebagai masukan hanya-baca.

## Fitur

- OverlayFS dan Magic Mount dapat digunakan bersama per modul maupun per jalur.
- Aturan jalur lebih diprioritaskan daripada nilai bawaan modul, dan nilai bawaan modul lebih diprioritaskan daripada nilai bawaan global.
- OverlayFS mendukung mode penyimpanan tmpfs dan ext4.
- Untuk staging ext4, KernelSU menggunakan ioctl resmi untuk menyembunyikan node sysfs; APatch dan lingkungan non-KSU lain menggunakan LKM kompatibilitas bawaan secara default.
- Magic Mount mendukung file, direktori, tautan simbolis, `.replace`, dan semantik whiteout.
- WebUI menyediakan antarmuka MD3 (bawaan) dan Miuix.
- arm64, armv7, dan x86_64 didukung; penginstal otomatis memilih biner yang sesuai.

## Instalasi

Unduh ZIP dari [Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases), lalu instal melalui pengelola KernelSU atau APatch. Pada instalasi pertama, gunakan tombol volume untuk memilih backend bawaan. Pembaruan tetap mempertahankan `/data/adb/hybrid-mount/config.toml`.

## Konfigurasi

Konfigurasi bawaan:

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

Jalur aturan ditulis relatif terhadap root modul. Aturan tingkat modul dan jalur juga dapat menggunakan `ignore`; backend bawaan global hanya menerima `overlay` atau `magic`. Jalur file yang sama tidak dapat ditetapkan ke kedua backend mount. Direktori biasa dapat dibagikan sebagai node struktur oleh kedua backend, sedangkan konflik file, tipe, atau `.replace` akan langsung menggagalkan tahap perencanaan saat boot. Perubahan konfigurasi berlaku setelah perangkat dimulai ulang.

Perutean ini tidak mengubah pemeriksaan kemampuan `CONFIG_TMPFS_XATTR` yang sudah ada. Pada KernelSU, instalasi menghapus seluruh direktori `lkm/` milik modul dan saat berjalan hanya menggunakan ioctl resmi `NukeExt4Sysfs`. Instalasi APatch dan lingkungan non-KSU lain mempertahankan LKM dan secara default mencobanya setelah staging ext4 terpasang. File `.ko` bawaan hanya mendukung aarch64. Pemilihan otomatis memerlukan kecocokan persis antara lini kernel dan tag Android/GKI; kombinasi yang tidak dikenal akan ditolak. LKM prabangun tetap harus divalidasi kompatibilitas ABI-nya pada perangkat fisik yang sesuai. Jika perangkat mengalami crash selama `insmod`, penanda pemutus sirkuit persisten akan mencegah LKM dimuat kembali pada boot berikutnya tanpa menonaktifkan fungsi Hybrid Mount lainnya. Lihat [`module/lkm/README.md`](../module/lkm/README.md) untuk matriks dukungan, checksum, sumber, dan lisensi.

## Umpan balik

Sebelum menginstal atau melaporkan masalah, baca [Pemberitahuan Penggunaan](../USAGE_NOTICE.md). Sertakan bugreport KernelSU/APatch, versi modul, dan langkah reproduksi. Hubungi kami melalui [GitHub Issues](https://github.com/Hybrid-Mount/meta-hybrid_mount/issues) atau [grup Telegram](https://t.me/hybridmountchat).

## Bahasa / Languages

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

## Lisensi

- Inti (Rust dan skrip modul): GPL-3.0-only (lihat [`LICENSE`](../LICENSE)).
- WebUI: Apache-2.0 (lihat [`webui/LICENSE`](../webui/LICENSE)).
- LKM sysfs ext4 opsional (sumber dan file `.ko` prabangun): GPL-2.0-only, diturunkan dari [Mountify](https://github.com/backslashxx/mountify); lihat [`module/lkm/README.md`](../module/lkm/README.md) dan [`module/lkm/src/LICENSE`](../module/lkm/src/LICENSE).
