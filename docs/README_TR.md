# ReHybrid-Mount

ReHybrid-Mount, Hybrid Mount'un sıfırdan yeniden inşasıdır ve şu davranış referanslarına dayanır:

- **Magic Mount**: [`Tools-cx-app/meta-magic_mount-rs`](https://github.com/Tools-cx-app/meta-magic_mount-rs) referans alınır (master `8b85c9e`; PR #152 şimdilik kabul edilmez).
- **OverlayFS**: Hybrid Mount v4.2.0 (tag `e20f9c19`) overlay davranışı referans alınır.
- **WebUI**: Vue 3 çift arayüz — Miuix (varsayılan) + MD3 (mevcut arayüz deneyimi korunur).
- **Ön uç / arka uç etkileşimi**: `kernelsu.exec` doğrudan CLI çağrısı yapar; arka plan hizmeti / HTTP / SSE yoktur.
- **Modül dizini kuralı**: `/data/adb/modules/<id>/system/**` hiçbir aşamada taşınmamalı, birleştirilmemeli veya silinmemelidir.

> v6.0.0'dan itibaren `dev`, sıfırdan yeniden oluşturulan geliştirme hattıdır.
> v4.2.0 ve önceki uygulamalar sürüm etiketleri, `archive/*` dalları ve tam Git geçmişi üzerinden erişilebilir; güncel kaynak ağacına geri taşınmaz.

## Diller / Languages

- [简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/README.md)
- [Türkçe](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_TR.md)

## Geçmiş ve Katkıda Bulunanlar

Yeni `dev` kaynak ağacı eski daemon, Kasumi, Lite/Nano flavor ve React WebUI kodunu içermez.
Buna rağmen eski geliştirme hattı Git geçmişine bağlanır; önceki commit'ler, yazarlar ve katkılar
izlenebilir kalır. WebUI katkıda bulunanlar listesi de botları hariç tutarak depo geçmişinden dinamik olarak oluşturulur.

Kurulumdan ve hata bildiriminden önce [`USAGE_NOTICE.md`](../USAGE_NOTICE.md) dosyasını okuyun.

## Lisans

- Çekirdek (Rust, modül betikleri): GPL-3.0-only (bkz. `LICENSE`).
- WebUI: Apache-2.0 (bkz. `webui/LICENSE`).

## Planlama

Mimari kararlar ve özgün uygulama planı için `REHYBRID_MOUNT_PLAN.md` dosyasına bakın.
