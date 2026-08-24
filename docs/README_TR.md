# Hybrid Mount

Hybrid Mount, KernelSU ve APatch için karma bir bağlama metamodülüdür. Açılış sırasında diğer modülleri tarar; genel, modül ve yol kurallarına göre her girdiyi OverlayFS, Magic Mount veya yoksayma moduna yönlendirir. Modül kaynak dizinleri her zaman salt okunur girdi olarak kabul edilir.

## Özellikler

- OverlayFS ve Magic Mount modül veya yol düzeyinde birlikte kullanılabilir.
- Yol kuralları modül varsayılanından, modül varsayılanı da genel varsayılandan önceliklidir.
- OverlayFS, tmpfs ve ext4 depolama modlarını destekler.
- Magic Mount; dosya, dizin, sembolik bağlantı, `.replace` ve whiteout semantiğini destekler.
- WebUI, Miuix (varsayılan) ve MD3 arayüzlerini sunar.
- arm64, armv7 ve x86_64 mimarileri desteklenir.

## Kurulum

ZIP dosyasını [Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases) sayfasından indirin ve KernelSU veya APatch yöneticisiyle kurun. İlk kurulumda varsayılan backend ses tuşlarıyla seçilebilir. Güncellemeler `/data/adb/hybrid-mount/config.toml` dosyasını korur.

## Geri bildirim

Kurulumdan veya hata bildiriminden önce [kullanım bildirimini](../USAGE_NOTICE.md) okuyun. Hata raporuna KernelSU/APatch bugreport, modül sürümü ve yeniden üretme adımlarını ekleyin. Bize [GitHub Issues](https://github.com/Hybrid-Mount/meta-hybrid_mount/issues) veya [Telegram grubu](https://t.me/hybridmountchat) üzerinden ulaşabilirsiniz.

## Diller / Languages

- [简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/README.md)
- [Türkçe](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_TR.md)

## Lisans

- Çekirdek (Rust ve module betikleri): GPL-3.0-only (bkz. [`LICENSE`](../LICENSE)).
- WebUI: Apache-2.0 (bkz. [`webui/LICENSE`](../webui/LICENSE)).
