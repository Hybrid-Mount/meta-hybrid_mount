# Hybrid Mount

<img src="../icon.svg" alt="Hybrid Mount logo" align="right" width="120" />

Hybrid Mount, KernelSU ve APatch için karma bir bağlama metamodülüdür. Açılış sırasında diğer modülleri tarar; genel, modül ve yol kurallarına göre her girdiyi OverlayFS, Magic Mount, VFS veya yoksayma moduna yönlendirir. Modül kaynak dizinleri her zaman salt okunur girdi olarak kabul edilir.

## Özellikler

- OverlayFS, Magic Mount ve VFS modül veya yol düzeyinde birlikte kullanılabilir.
- Yol kuralları modül varsayılanından, modül varsayılanı da genel varsayılandan önceliklidir.
- OverlayFS, tmpfs ve ext4 depolama modlarını destekler.
- ext4 hazırlama alanında KernelSU, sysfs düğümlerini gizlemek için resmi ioctl'u kullanır; APatch ve diğer KSU dışı ortamlar varsayılan olarak birlikte gelen uyumluluk LKM'sini kullanır.
- Magic Mount; dosya, dizin, sembolik bağlantı, `.replace` ve whiteout semantiğini destekler.
- VFS, enjeksiyon kurallarını keyring üzerinden Hybrid Mount'un kendi VFS alt sistemine (`hybridmount` modülü) gönderir. Bağımsız bir uygulamadır; NoMount çekirdeği veya nm CLI'si ile birlikte çalışmaz. Sürümler kaynak kodu ve desteklenen her Android/GKI hedefi için önceden derlenmiş arm64 modülü içerir; çekirdek bunu barındırmıyorsa önyüklemede yüklenir. Başarısız olursa davranış `vfs_strict` ile belirlenir. VFS gerçek bir bağlama değildir.
- WebUI, MD3 (varsayılan) ve Miuix arayüzlerini sunar.
- arm64, armv7 ve x86_64 mimarileri desteklenir; yükleyici uygun ikili dosyayı otomatik olarak seçer.

## Kurulum

ZIP dosyasını [Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases) sayfasından indirin ve KernelSU veya APatch yöneticisiyle kurun. İlk kurulumda varsayılan backend ses tuşlarıyla seçilebilir. Güncellemeler `/data/adb/hybrid-mount/config.toml` dosyasını korur.

## Yapılandırma

Varsayılan yapılandırma:

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

Kural yolları modül kök dizinine göre yazılır. Modül ve yol düzeyindeki kurallar `ignore` değerini de kullanabilir; genel varsayılan backend `overlay`, `magic` veya `vfs` değerini kabul eder. VFS gerçek bir bağlama değil, bir enjeksiyon yoludur. Dosya, tür veya `.replace` çakışmaları açılış planlama aşamasını doğrudan hatayla durdurur. Yapılandırma değişiklikleri yeniden başlatmadan sonra geçerli olur.

Bu yönlendirme, projenin mevcut `CONFIG_TMPFS_XATTR` yetenek denetimini değiştirmez. KernelSU kurulumunda modülün tüm `lkm/` dizini silinir ve çalışma zamanında yalnızca resmi `NukeExt4Sysfs` ioctl'u kullanılır. APatch ve diğer KSU dışı kurulumlar LKM'yi korur ve ext4 hazırlama alanı bağlandıktan sonra varsayılan olarak kullanmayı dener. Birlikte gelen `.ko` dosyaları yalnızca aarch64'ü destekler. Otomatik seçim için çekirdek serisi ile Android/GKI etiketinin tam olarak eşleşmesi gerekir; bilinmeyen kombinasyonlar reddedilir. Önceden derlenmiş LKM'lerin ABI uyumluluğu yine de ilgili gerçek cihazda doğrulanmalıdır. Cihaz `insmod` sırasında çökerse kalıcı bir devre kesici işareti, Hybrid Mount'un diğer işlevlerini korurken LKM'nin sonraki açılışta yeniden yüklenmesini önler. Destek matrisi, sağlama toplamları, kaynaklar ve lisanslar için [`module/lkm/README.md`](../module/lkm/README.md) dosyasına bakın.

## Geri bildirim

Kurulumdan veya hata bildiriminden önce [kullanım bildirimini](../USAGE_NOTICE.md) okuyun. Hata raporuna KernelSU/APatch bugreport, modül sürümü ve yeniden üretme adımlarını ekleyin. Bize [GitHub Issues](https://github.com/Hybrid-Mount/meta-hybrid_mount/issues) veya [Telegram grubu](https://t.me/hybridmountchat) üzerinden ulaşabilirsiniz.

## Diller / Languages

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

## Lisans

- Çekirdek (Rust ve module betikleri): GPL-3.0-only (bkz. [`LICENSE`](../LICENSE)).
- WebUI: Apache-2.0 (bkz. [`webui/LICENSE`](../webui/LICENSE)).
- İsteğe bağlı ext4 sysfs LKM (kaynak ve önceden derlenmiş `.ko` dosyaları): [Mountify](https://github.com/backslashxx/mountify) tabanlı GPL-2.0-only; [`module/lkm/README.md`](../module/lkm/README.md) ve [`module/lkm/src/LICENSE`](../module/lkm/src/LICENSE) dosyalarına bakın.
- VFS alt sistemi (`hybridmount` modülü): [NoMount](https://github.com/maxsteeel/nomount) tabanlı GPL-2.0-only; [`module/vfs/README.md`](../module/vfs/README.md), [`module/vfs/src/LICENSE`](../module/vfs/src/LICENSE) ve [THIRD_PARTY.md](../THIRD_PARTY.md) dosyalarına bakın.
