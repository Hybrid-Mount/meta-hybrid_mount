# Hybrid Mount

<img src="../icon.svg" alt="Hybrid Mount logo" align="right" width="120" />

Hybrid Mount adalah metamodul mount hibrida untuk KernelSU dan APatch. Saat boot, modul ini memindai modul lain lalu memilih OverlayFS, Magic Mount, VFS, atau abaikan untuk setiap entri berdasarkan aturan global, modul, dan jalur. Direktori sumber modul selalu diperlakukan sebagai masukan hanya-baca.

## Fitur

- OverlayFS, Magic Mount, dan VFS dapat digunakan bersama per modul maupun per jalur.
- Aturan jalur lebih diprioritaskan daripada nilai bawaan modul, dan nilai bawaan modul lebih diprioritaskan daripada nilai bawaan global.
- OverlayFS mendukung mode penyimpanan tmpfs dan ext4.
- Untuk staging ext4, KernelSU menggunakan ioctl resmi untuk menyembunyikan node sysfs; APatch dan lingkungan non-KSU lain menggunakan LKM kompatibilitas bawaan secara default.
- Magic Mount mendukung file, direktori, tautan simbolis, `.replace`, dan semantik whiteout.
- VFS mengirim aturan injeksi ke subsistem VFS milik Hybrid Mount (modul `hybridmount`) melalui keyring. Ini implementasi independen dan tidak berinteroperasi dengan kernel NoMount maupun CLI nm-nya. Rilis menyertakan sumber dan modul arm64 prakompilasi untuk setiap target Android/GKI yang didukung, dimuat saat boot bila kernel tidak membawanya; jika gagal, perilakunya mengikuti `vfs_strict`. VFS bukan mount sungguhan.
- WebUI menyediakan antarmuka MD3 (bawaan) dan Miuix.
- arm64, armv7, dan x86_64 didukung; penginstal otomatis memilih biner yang sesuai.

## Instalasi

Unduh ZIP dari [Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases), lalu instal melalui pengelola KernelSU atau APatch. Pada instalasi pertama, gunakan tombol volume untuk memilih backend bawaan. Pembaruan tetap mempertahankan `/data/adb/hybrid-mount/config.toml`.

## Konfigurasi

Konfigurasi bawaan:

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

Jalur aturan ditulis relatif terhadap root modul. Aturan tingkat modul dan jalur juga dapat menggunakan `ignore`; backend bawaan global menerima `overlay`, `magic`, atau `vfs`. VFS adalah jalur injeksi, bukan mount nyata. Konflik file, tipe, atau `.replace` akan langsung menggagalkan tahap perencanaan saat boot. Perubahan konfigurasi berlaku setelah perangkat dimulai ulang.

Perutean ini tidak mengubah pemeriksaan kemampuan `CONFIG_TMPFS_XATTR` yang sudah ada. Pada KernelSU, instalasi menghapus seluruh direktori `lkm/` milik modul dan saat berjalan hanya menggunakan ioctl resmi `NukeExt4Sysfs`. Instalasi APatch dan lingkungan non-KSU lain mempertahankan LKM dan secara default mencobanya setelah staging ext4 terpasang. File `.ko` bawaan hanya mendukung aarch64. Pemilihan otomatis memerlukan kecocokan persis antara lini kernel dan tag Android/GKI; kombinasi yang tidak dikenal akan ditolak. LKM prabangun tetap harus divalidasi kompatibilitas ABI-nya pada perangkat fisik yang sesuai. Jika perangkat mengalami crash selama `insmod`, penanda pemutus sirkuit persisten akan mencegah LKM dimuat kembali pada boot berikutnya tanpa menonaktifkan fungsi Hybrid Mount lainnya. Lihat [`module/lkm/README.md`](../module/lkm/README.md) untuk matriks dukungan, checksum, sumber, dan lisensi.

## Backend VFS

VFS adalah jalur injeksi sisi kernel milik Hybrid Mount, yang dikendalikan melalui keyring oleh modul `hybridmount`. Ini adalah implementasi independen dan tidak berinteroperasi dengan NoMount.

**Cara penyedia diidentifikasi.** Keputusan boot bertumpu pada pemeriksaan hanya-baca terhadap jenis kunci kernel `hybridmount`: jika jenis kunci itu menjawab dengan versi yang didukung, penyedia dapat digunakan. Secara terpisah, `vfs-doctor` mengklasifikasikan bagaimana penyedia hadir — entri di `/proc/modules` berarti ada modul yang dapat dimuat yang mendaftarkannya, direktori `/sys/module/hybridmount` tanpa entri tersebut berarti jenis kunci dikompilasi ke dalam image kernel, dan tidak ada keduanya berarti tidak ada penyedia yang tersedia. Pemeriksaan bersifat hanya-baca, sehingga `status` dan `vfs-doctor` tidak pernah memicu `insmod`.

**Logika boot.** Jika jenis kunci menjawab dengan versi yang didukung, penyedia diikat dan tidak ada yang dimuat. Jika tidak ada aturan yang memilih VFS, modul bawaan juga tidak dimuat. Jika ada aturan yang memilih VFS sementara pemeriksaan tidak memberi jawaban, pipeline memilih modul bawaan yang cocok persis dengan lini kernel dan tag Android/GKI, memuatnya, lalu memeriksa ulang; jika tetap tidak tersedia, setiap aturan `vfs` diturunkan menjadi `ignore`, atau menggagalkan boot saat `vfs_strict = true`. Pemuatan berjalan sebelum rencana mount dibangun, karena perencanaan menulis ulang aturan `vfs` menjadi `ignore` selagi penyedia tidak memberi jawaban dan eksekutor kemudian akan kembali lebih awal. Penanda pemutus sirkuit ditulis sebelum `insmod` dan dihapus saat upaya itu kembali, sehingga hanya crash kernel yang meninggalkannya; boot berikutnya kemudian menolak upaya otomatis ulang sampai penanda itu dihapus secara manual.

**Mengintegrasikan VFS ke dalam kernel.** Rilis menyertakan modul aarch64 prabangun untuk setiap target Android/GKI yang didukung dan memuatnya secara otomatis, sehingga kernel tersebut tidak memerlukan langkah integrasi. Bangun modul itu menyatu ke dalam kernel bila Anda ingin menghindari `insmod`, atau bila lini kernel Anda tidak memiliki versi prabangun. Dari akar pohon kernel:

```sh
curl -LSs "https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/dev/module/vfs/setup.sh" | bash
```

`--cleanup`:

```sh
curl -LSs "https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/dev/module/vfs/setup.sh" | bash -s -- --cleanup
```

Perintah ini menyalin kode sumber ke `fs/hybridmount/` dan menambahkannya ke `fs/Makefile` dan `fs/Kconfig`; aktifkan `CONFIG_HYBRIDMOUNT=y` untuk membangunnya menyatu atau `=m` untuk membangunnya sebagai modul. `bash -s -- --cleanup` mengembalikan semua perubahan. Pohon yang sudah mengintegrasikan NoMount akan ditolak: kedua implementasi membajak operasi inode dan kernel tidak akan mencegah keduanya berdampingan, karena keduanya mendaftarkan jenis kunci yang berbeda.

**Diagnosis.** `/data/adb/modules/hybrid_mount/hybrid-mount vfs-doctor` melaporkan status keberadaan, versi yang dijawab jenis kunci, versi yang didukung, dan alasan penyedia tidak dapat digunakan bila memang demikian.

## Umpan balik

Sebelum menginstal atau melaporkan masalah, baca [Pemberitahuan Penggunaan](../USAGE_NOTICE.md). Sertakan bugreport KernelSU/APatch, versi modul, dan langkah reproduksi. Hubungi kami melalui [GitHub Issues](https://github.com/Hybrid-Mount/meta-hybrid_mount/issues) atau [grup Telegram](https://t.me/hybridmountchat).

## Bahasa / Languages

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

## Ucapan terima kasih

- Terima kasih kepada [Anatdx](https://github.com/Anatdx)
- Terima kasih kepada [Tools-cx-app](https://github.com/Tools-cx-app)
- Terima kasih kepada [KernelSU](https://github.com/tiann/KernelSU)
- Terima kasih kepada [MKSU dari 5ec1cff](https://github.com/5ec1cff/KernelSU)
- Terima kasih kepada [ReSukiSU](https://github.com/ReSukiSU/ReSukiSU)
- Terima kasih kepada [meta-magic_mount-rs](https://github.com/Tools-cx-app/meta-magic_mount-rs)
- Terima kasih kepada [NoMount](https://github.com/maxsteeel/nomount)

## Lisensi

- Inti (Rust dan skrip modul): GPL-3.0-only (lihat [`LICENSE`](../LICENSE)).
- WebUI: Apache-2.0 (lihat [`webui/LICENSE`](../webui/LICENSE)).
- LKM sysfs ext4 opsional (sumber dan file `.ko` prabangun): GPL-2.0-only, diturunkan dari [Mountify](https://github.com/backslashxx/mountify); lihat [`module/lkm/README.md`](../module/lkm/README.md) dan [`module/lkm/src/LICENSE`](../module/lkm/src/LICENSE).
- Subsistem VFS (modul `hybridmount`): GPL-2.0-only, diturunkan dari [NoMount](https://github.com/maxsteeel/nomount); lihat [`module/vfs/README.md`](../module/vfs/README.md), [`module/vfs/src/LICENSE`](../module/vfs/src/LICENSE), dan [THIRD_PARTY.md](../THIRD_PARTY.md).
