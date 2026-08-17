# ReHybrid-Mount

ReHybrid-Mount, Hybrid Mount'un sıfırdan yeniden inşasıdır ve şu davranış referanslarına dayanır:

- **Magic Mount**: [`Tools-cx-app/meta-magic_mount-rs`](https://github.com/Tools-cx-app/meta-magic_mount-rs) referans alınır (master `8b85c9e`; PR #152 şimdilik kabul edilmez).
- **OverlayFS**: Hybrid Mount v4.2.0 (tag `e20f9c19`) overlay davranışı referans alınır.
- **WebUI**: Vue 3 çift arayüz — Miuix (varsayılan) + MD3 (mevcut arayüz deneyimi korunur).
- **Ön uç / arka uç etkileşimi**: `kernelsu.exec` doğrudan CLI çağrısı yapar; arka plan hizmeti / HTTP / SSE yoktur.
- **Modül dizini kuralı**: `/data/adb/modules/<id>/system/**` hiçbir aşamada taşınmamalı, birleştirilmemeli veya silinmemelidir.

> Güncel `rehybrid-mount` dalı bir orphan daldır ve sıfırdan yeniden inşa edilmektedir.
> Eski uygulama orijinal `main` / `dev` / `archive/*` dallarında korunmaktadır.

## Diller / Languages

- [English](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/rehybrid-mount/README.md)
- [Türkçe](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/rehybrid-mount/docs/README_TR.md)

## Lisans

- Çekirdek (Rust, modül betikleri): GPL-3.0-only (bkz. `LICENSE`).
- WebUI: Apache-2.0 (bkz. `webui/LICENSE`).

## Planlama

Uygulama planının tamamı için `REHYBRID_MOUNT_PLAN.md` dosyasına bakın.
