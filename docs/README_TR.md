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
- arm64, armv7, x86_64 ve riscv64 mimarileri desteklenir; yükleyici uygun ikili dosyayı otomatik olarak seçer. riscv64 derlemesi Android NDK r27 veya üstünü gerektirir.

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

## VFS arka ucu

VFS, Hybrid Mount'un çekirdek tarafındaki kendi enjeksiyon yoludur ve `hybridmount` modülü tarafından keyring üzerinden yönetilir. Bağımsız bir uygulamadır ve NoMount ile birlikte çalışmaz.

**Sağlayıcı nasıl belirlenir.** Önyükleme kararı yalnızca `hybridmount` çekirdek anahtar türünün salt okunur yoklamasına dayanır: desteklenen bir sürümle yanıt verirse sağlayıcı kullanılabilir. Ayrıca `vfs-doctor`, sağlayıcının nasıl mevcut olduğunu sınıflandırır — `/proc/modules` içindeki bir girdi, onu yüklenebilir bir modülün kaydettiği anlamına gelir; bu girdi olmadan var olan bir `/sys/module/hybridmount` dizini, çekirdek imajına derlendiği anlamına gelir; ikisi de yoksa sağlayıcı yoktur. Yoklama salt okunurdur, bu nedenle `status` ve `vfs-doctor` hiçbir zaman bir `insmod` tetiklemez.

**Önyükleme mantığı.** Anahtar türü desteklenen bir sürümle yanıt verirse sağlayıcı bağlanır ve hiçbir şey yüklenmez. Hiçbir kural VFS'yi seçmiyorsa birlikte gelen modül de yüklenmez. Bir kural VFS'yi seçtiği hâlde yoklama sessiz kalırsa ardışık düzen, çekirdek serisi ve Android/GKI etiketiyle tam olarak eşleşen birlikte gelen modülü seçer, yükler ve yeniden yoklar; hâlâ kullanılamıyorsa her `vfs` kuralı `ignore` durumuna düşer veya `vfs_strict = true` olduğunda önyükleme başarısız olur. Yükleme, bağlama planı oluşturulmadan önce çalışır; çünkü sağlayıcı sessizken planlama `vfs` kurallarını `ignore` olarak yeniden yazar ve yürütücü de bu durumda erken döner. Devre kesici işareti `insmod` öncesinde yazılır ve girişim döndüğünde temizlenir; dolayısıyla geride yalnızca bir çekirdek çökmesi bırakır ve bir sonraki önyükleme, işaret elle silinene kadar otomatik yeniden denemeyi reddeder.

**VFS'yi çekirdeğe entegre etme.** Sürümler, desteklenen her Android/GKI hedefi için önceden derlenmiş bir aarch64 modülü sunar ve bunu otomatik olarak yükler; dolayısıyla bu çekirdekler entegrasyon adımı gerektirmez. `insmod` kullanmak istemiyorsanız veya çekirdek seriniz için önceden derlenmiş modül yoksa modülü çekirdeğe dahil edin. Bir çekirdek ağacının kök dizininden:

```sh
curl -LSs "https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/dev/module/vfs/setup.sh" | bash
```

`--cleanup`:

```sh
curl -LSs "https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/dev/module/vfs/setup.sh" | bash -s -- --cleanup
```

Bu, kaynakları `fs/hybridmount/` dizinine kopyalar ve `fs/Makefile` ile `fs/Kconfig` dosyalarına ekler; çekirdeğe dahil etmek için `CONFIG_HYBRIDMOUNT=y`, modül olarak derlemek için `=m` etkinleştirin. `bash -s -- --cleanup` tüm değişiklikleri geri alır. NoMount'un zaten entegre olduğu bir ağaç reddedilir: her iki uygulama da inode işlemlerini ele geçirir ve farklı anahtar türleri kaydettikleri için çekirdek bunların bir arada var olmasını engellemez.

**Tanılama.** `/data/adb/modules/hybrid_mount/hybrid-mount vfs-doctor`, varlık durumunu, anahtar türünün yanıtladığı sürümü, desteklenen sürümleri ve sağlayıcı kullanılamadığında bunun nedenini bildirir.

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

## Teşekkürler

- Teşekkürler [Anatdx](https://github.com/Anatdx)
- Teşekkürler [Tools-cx-app](https://github.com/Tools-cx-app)
- Teşekkürler [KernelSU](https://github.com/tiann/KernelSU)
- Teşekkürler [5ec1cff'in MKSU](https://github.com/5ec1cff/KernelSU)
- Teşekkürler [ReSukiSU](https://github.com/ReSukiSU/ReSukiSU)
- Teşekkürler [meta-magic_mount-rs](https://github.com/Tools-cx-app/meta-magic_mount-rs)
- Teşekkürler [NoMount](https://github.com/maxsteeel/nomount)

## Lisans

- Çekirdek (Rust ve module betikleri): GPL-3.0-only (bkz. [`LICENSE`](../LICENSE)).
- WebUI: Apache-2.0 (bkz. [`webui/LICENSE`](../webui/LICENSE)).
- İsteğe bağlı ext4 sysfs LKM (kaynak ve önceden derlenmiş `.ko` dosyaları): [Mountify](https://github.com/backslashxx/mountify) tabanlı GPL-2.0-only; [`module/lkm/README.md`](../module/lkm/README.md) ve [`module/lkm/src/LICENSE`](../module/lkm/src/LICENSE) dosyalarına bakın.
- VFS alt sistemi (`hybridmount` modülü): [NoMount](https://github.com/maxsteeel/nomount) tabanlı GPL-2.0-only; [`module/vfs/README.md`](../module/vfs/README.md), [`module/vfs/src/LICENSE`](../module/vfs/src/LICENSE) ve [THIRD_PARTY.md](../THIRD_PARTY.md) dosyalarına bakın.
