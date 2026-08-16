# Hybrid Mount

<img src="https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/main/icon.svg" align="right" width="120" />

![Language](https://img.shields.io/badge/Language-Rust-orange?style=flat-square&logo=rust)
![Platform](https://img.shields.io/badge/Platform-Android-green?style=flat-square&logo=android)
![License](https://img.shields.io/badge/License-GPL--3.0-blue?style=flat-square)
![Version](https://img.shields.io/github/v/tag/Hybrid-Mount/meta-hybrid_mount?label=Version&color=8A2BE2&style=flat-square)

Hybrid Mount, **KernelSU** ve **APatch** için bir bağlama orkestrasyon meta-modülüdür.
Modül dosyalarını iki bağlama arka ucuna sahip birleşik bir politika motoruyla Android bölümlerine birleştirir:

- **OverlayFS** — upper/work depolamasına sahip katmanlı bağlamalar.
- **Magic Mount** — yolları doğrudan değiştiren bind mount'lar.

Yerleşik **SolidJS WebUI**, grafiksel yönetim, canlı durum izleme ve yapılandırma düzenleme sağlar.

Sürümler iki türde yayımlanır. Ayrıntılar için [Derleme Türleri](#derleme-türleri) bölümüne bakın. Aksi belirtilmedikçe bu belge varsayılan Lite sürümünü anlatır.

**[English](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/README.md)** &nbsp; **[简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH.md)** &nbsp; **[繁體中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH_TW.md)** &nbsp; **[日本語](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_JP.md)** &nbsp; **[Español](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ES.md)** &nbsp; **[Italiano](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_IT.md)** &nbsp; **[Русский](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_RU.md)** &nbsp; **[Українська](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_UK.md)** &nbsp; **[Tiếng Việt](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_VI.md)** &nbsp; **[Türkçe](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_TR.md)**

---

## İçindekiler

- [Özellikler](#özellikler)
- [Derleme Türleri](#derleme-türleri)
- [Hızlı Başlangıç](#hızlı-başlangıç)
- [Bağlama Modları](#bağlama-modları)
- [WebUI](#webui)
- [Yapılandırma](#yapılandırma)
- [Politika Referansı](#politika-referansı)
- [CLI](#cli)
- [Mimari](#mimari)
- [Derleme](#derleme)
- [Late-load Desteği](#late-load-desteği)
- [Lisans](#lisans)

---

## Derleme Türleri

| Tür | İkili dosya | WebUI | Daemon / CLI | Kullanım senaryosu |
| --- | --- | --- | --- | --- |
| **Lite** | Evet | Evet | Evet | Varsayılan sürüm: WebUI, daemon, CLI, OverlayFS ve Magic Mount. |
| **Nano** | Evet | Hayır | Hayır | Yalnızca yapılandırma dosyasıyla bağlama isteyen minimalist kullanım. |

### Lite

Lite varsayılan sürümdür. SolidJS WebUI'yi, HTTP/SSE destekli Unix-socket daemon'unu, CLI'yi ve iki bağlama arka ucunu içerir. Yalnızca `control-plane` özellik setiyle derlenir:

```text
--no-default-features --features control-plane
```

### Nano

Nano, hiçbir Cargo özelliği etkin olmayan **yalnızca yapılandırma** sürümüdür. WebUI, daemon, CLI ve control-plane altyapısı pakete dahil edilmez.

- Önyüklemede `config.toml` okunur, bağlama planı uygulanır ve süreç sonlanır.
- Varsayılan mod `magic` olarak ayarlanır.
- Kurulumdaki ses tuşu seçimi, modül köklerine `overlay` veya `magic` işaretçisi yazar; işaretçi adları büyük/küçük harfe duyarsız eşleştirilir.
- Arka planda çalışan kalıcı bir Hybrid Mount süreci yoktur.

### Özellik Matrisi

| Özellik | Lite | Nano |
| --- | --- | --- |
| OverlayFS | Evet | İşaretçi tabanlı |
| Magic Mount | Evet | Evet, varsayılan |
| WebUI | Evet | Hayır |
| CLI | Evet | Hayır |
| Daemon (Unix + TCP/SSE) | Evet | Hayır |
| Çalışma zamanı yapılandırma uygulaması | Hayır (sonraki açılış için kaydedilir) | Hayır |
| Cargo özellikleri | yalnızca `control-plane` | yok |

## Özellikler

- **İki arka uç, tek politika motoru** — yolları OverlayFS veya Magic Mount'a ayrıntılı biçimde atayın.
- **Belirlenimci planlama** — çakışmalar rastgele önyükleme hatası olmak yerine planlama sırasında bulunur.
- **Yerleşik WebUI** — modülleri yönetin, yapılandırmayı düzenleyin ve çalışma zamanı durumunu izleyin.
- **Çalışma zamanı güncellemeleri** — doğrulanmış yapılandırma değişiklikleri kaydedilir ve bir sonraki açılışta etkili olur. WebUI bunu açıkça bildirir; değişikliklerin anında uygulandığı izlenimi vermez.
- **Açık hata raporlama** — geçersiz durum ve yapılandırma hataları gizlenmeden bildirilir.
- **Otomasyona uygun** — Unix socket üzerinden JSON protokolü ve HTTP API.

---

## Hızlı Başlangıç

### Kurulum

1. Cihaza [KernelSU](https://kernelsu.org/) veya [APatch](https://apatch.dev/) kurun.
2. [GitHub Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases) sayfasından son Lite veya Nano ZIP'ini indirin.
3. ZIP'i root yöneticisinin modül yükleyicisiyle kurun.
4. Yeni Lite kurulumunda varsayılan modu seçin: Ses Açma OverlayFS, Ses Kısma Magic Mount; 10 saniyelik zaman aşımı OverlayFS seçer. Nano bu adımı atlar.
5. Cihazı yeniden başlatın.

### Kurulum Sonrası

```bash
# Çalışma zamanı durumunu kontrol et
hybrid-mount daemon status

# Algılanan modülleri listele
hybrid-mount api modules-list
```

WebUI için KernelSU veya APatch yöneticisinde Hybrid Mount modülüne dokunun. Yönetici WebUI'yi gömülü WebView içinde açar.

### Bir Modülün Bağlama Modunu Değiştirme

```toml
# /data/adb/hybrid-mount/config.toml
[rules.my_module]
default_mode = "magic"

[rules.my_module.paths]
"system/bin/problematic_binary" = "ignore"
```

---

## Bağlama Modları

| Mod | Arka uç | En uygun kullanım |
| --- | --- | --- |
| `overlay` | OverlayFS | Çakışmadan dosya ekleyen veya değiştiren modüller; varsayılan mod. |
| `magic` | Bind mount | Doğrudan dosya bazında değiştirme gereken modüller. |
| `ignore` | — | Belirli yolları bağlama işleminin dışında bırakmak. |

### OverlayFS Depolama Modları

- `ext4` (varsayılan) — her bağlama çalışması için yeni bir ext4 staging imajı oluşturur. Overlay xattr destekler; bağlamalar tamamlanınca imaj kaldırılır.
- `tmpfs` — RAM tabanlı ve geçicidir; yeniden başlatmada kaybolur.

```toml
overlay_mode = "ext4"
```

---

## WebUI

WebUI, daemon tarafından yerel TCP socket üzerinden HTTP/SSE ile sunulur. CLI ve otomasyon istemcileri Unix socket kullanır. Erişim URL'si başlangıçta logcat'e yazılır.

### Yetenekler

- Canlı bağlama istatistikleri, etkin bölümler, depolama modu ve daemon durumu.
- Algılanan modülleri ve etkin bağlama modlarını yönetme.
- Doğrulamalı `config.toml` düzenleyicisi.

### Dil Desteği

WebUI şu dilleri içerir: İngilizce, İspanyolca, İtalyanca, Japonca, Rusça, Ukraynaca, Vietnamca, Basitleştirilmiş/Geleneksel Çince, Endonezce (`id-ID`) ve Türkçe (`tr-TR`).

---

## Yapılandırma

Varsayılan yol: `/data/adb/hybrid-mount/config.toml`.

| Alan | Tür | Varsayılan | Açıklama |
| --- | --- | --- | --- |
| `moduledir` | string | `/data/adb/modules` | Modül kaynak dizini. |
| `mountsource` | string | otomatik | Çalışma zamanı kaynak etiketi (`KSU`, `APatch`). |
| `overlay_mode` | `ext4` \| `tmpfs` | `ext4` | Overlay upper/work depolama modu. |
| `disable_umount` | bool | `false` | Umount işlemlerini atlar; yalnızca hata ayıklama için. |
| `default_mode` | `overlay` \| `magic` | `overlay` | Genel varsayılan politika. |
| `daemon_startup_mode` | `on-demand` \| `persistent` | `on-demand` | Daemon başlatma davranışı. |
| `rules` | map | `{}` | Modül ve yol bazında politika. |

```toml
moduledir = "/data/adb/modules"
mountsource = "KSU"
overlay_mode = "ext4"
disable_umount = false
default_mode = "overlay"

[rules.example_module]
default_mode = "magic"

[rules.example_module.paths]
"system/etc/hosts" = "ignore"
```

---

## Politika Referansı

### Öncelik

Bir yol için etkin mod şu sırayla çözülür:

1. En uzun eşleşen yol geçersiz kılması.
2. Modülün `default_mode` değeri.
3. Genel `default_mode`.

### Davranış Matrisi

| Mod | Modül etkin | Sonuç |
| --- | --- | --- |
| `overlay` | Evet | OverlayFS katmanına eklenir. |
| `magic` | Evet | Magic Mount ağacına eklenir. |
| `ignore` | Evet | Yol atlanır. |
| herhangi biri | Hayır | Modül atlanır. |

### Modül İşaretçi Dosyaları

| İşaretçi | Konum | Etki |
| --- | --- | --- |
| `disable` | Modül kökü | Modülü planın dışında bırakır. |
| `remove` | Modül kökü | Kaldırılmakta olan modülü dışlar. |
| `skip_mount` | Modül kökü | Modülün bağlama işlemini atlar. |
| `overlay` / `magic` | Modül kökü, Nano | Nano bağlama arka ucunu seçer. |
| `.replace` | Modül dizini | Dizin değiştirme/opaque semantiğini uygular. |

İşaretçi adları ASCII harfleri için büyük/küçük harfe duyarsız eşleştirilir; `DISABLE`, `Disable` ve `disable` aynı işaretçi kabul edilir.

---

## CLI

```text
hybrid-mount [OPTIONS] [COMMAND]
```

Genel seçenekler:

| Bayrak | Açıklama |
| --- | --- |
| `-c, --config <PATH>` | Özel yapılandırma dosyası yolu. |

Başlıca komutlar:

| Komut | Açıklama |
| --- | --- |
| `gen-config` | Varsayılan yapılandırma dosyasını oluşturur. |
| `logs` | Son daemon günlüklerini yazdırır. |
| `api storage` | Depolama modunu sorgular (ext4/tmpfs). |
| `api mount-stats` | Bağlama istatistiklerini yazdırır. |
| `api mount-topology` | Bağlama topoloji ağacını yazdırır. |
| `api partitions` | Yönetilen bölümleri listeler. |
| `api system-info` | Sistem bilgilerini yazdırır. |
| `api version` | Daemon sürümünü yazdırır. |
| `api config-get` | Etkin yapılandırmayı JSON olarak yazdırır. |
| `api config-set --config <JSON>` | Tam yapılandırmayı değiştirir (sonraki açılışta uygulanır). |
| `api config-patch --patch <JSON>` | Yapılandırmaya yama uygular (sonraki açılışta uygulanır; `--apply-runtime` kullanım dışı bir no-op'tur). |
| `api config-reset` | Yapılandırmayı varsayılana sıfırlar. |
| `api modules-list` | Algılanan modülleri listeler. |
| `api modules-apply --modules <JSON>` | Modül modu değişikliklerini uygular. |
| `api features` | Desteklenen özellikleri listeler. |
| `api kernel-uname` | Çekirdek uname bilgisini yazdırır. |
| `api open-url --url <URL>` | Cihazda URL açar. |
| `api reboot` | Cihazı yeniden başlatır. |
| `daemon launch` | Daemon'u ön planda başlatır. |
| `daemon serve` | Daemon'u başlatır (servis modu). |
| `daemon ping` | Daemon canlılığını denetler. |
| `daemon webui-start` | Yalnızca WebUI'yi başlatır. |
| `daemon stop` | Daemon'u durdurur. |
| `daemon status` | Daemon çalışma zamanı durumunu sorgular. |

---

## Mimari

```text
config.toml
    │
    ▼
Modül envanteri
    │
    ▼
Politika çözümleme ve planlama
    │
    ├── OverlayFS
    └── Magic Mount
    │
    ▼
Çalışma zamanı durumu ve WebUI / CLI
```

Kaynak düzeni:

```text
src/
├── conf/           Yapılandırma ve CLI şemaları
├── core/           Envanter, planlama, executor, daemon ve runtime state
├── domain/         Politika modelleri ve çözümleme
├── mount/          OverlayFS, Magic Mount ve custom bind uygulaması
├── sys/            Düşük seviyeli Linux/Android işlemleri
└── utils/          Günlükleme, doğrulama ve yol yardımcıları

webui/              SolidJS WebUI
module/             Android modül komut dosyaları ve paket varlıkları
xtask/              Derleme, lint ve paketleme aracı
```

---

## Derleme

### Ön Koşullar

- `rust-toolchain.toml` içinde sabitlenmiş Rust nightly.
- Android NDK r27+ ve `cargo-ndk`.
- WebUI için Node.js 20.19+ veya 22.12+ ve pnpm 10.34.5.

### Komutlar

```bash
# Lite yayın paketi
cargo run -p xtask -- build --release --flavor lite

# Nano yayın paketi
cargo run -p xtask -- build --release --flavor nano

# Yalnızca ikili dosya
cargo run -p xtask -- build --release --skip-webui

# Yerel arm64 / Nano hata ayıklama
./scripts/build-local.sh
./scripts/build-local.sh --nano

# WebUI geliştirme sunucusu
cd webui && pnpm install && pnpm dev

# Tam yerel doğrulama
cargo run -p xtask -- lint
```

CI; rustfmt, Clippy `-D warnings`, workspace ve Lite/Nano testleri, WebUI lint/test ve lisans başlıklarını denetler.

---

## Late-load Desteği

KernelSU `KSU_LATE_LOAD=1` ile geç yükleme/locked-bootloader akışını kullanabilir. `module/emulated-soft-reboot.sh`, yeniden bağlamadan önce önceki çalışmanın bağlamalarını ayırmak için `hybrid-mount emulated-soft-reboot` komutunu çağırır. Aynı temizlik başlangıçta da yapılır; böylece bağlamalar üst üste birikmez.

Temizlik yalnızca Hybrid Mount'a kesin olarak ait olduğu kanıtlanan bağlamaları işler: seçeneklerinde proje çalışma/veri dizini bulunan overlay'ler, tam olarak `/mnt/hm_<10 karakter>` veya `/debug_ramdisk/hm_<10 karakter>` biçimindeki çalışma alanları ve çalışma zamanı durumunda saklanan kesin Magic/custom hedefleri. Paylaşılan KernelSU/APatch `mountsource` değeri sahiplik kanıtı olarak kullanılmaz.

---

## Lisans

Bu proje [GNU General Public License v3.0](../LICENSE) altında lisanslanmıştır.
