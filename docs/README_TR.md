# Hybrid Mount

<img src="https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/main/icon.svg" align="right" width="120" />



![Language](https://img.shields.io/badge/Language-Rust-orange?style=flat-square&logo=rust)




![Platform](https://img.shields.io/badge/Platform-Android-green?style=flat-square&logo=android)




![License](https://img.shields.io/badge/License-Apache--2.0-blue?style=flat-square)




![Version](https://img.shields.io/github/v/tag/Hybrid-Mount/meta-hybrid_mount?label=Version&color=8A2BE2&style=flat-square)



Hybrid Mount, **KernelSU** ve **APatch** için bir bağlama (mount) orkestrasyon meta-modülüdür.
Modül dosyalarını, üç bağlama arka ucuyla desteklenen birleşik bir politika motoru aracılığıyla Android bölümlerine (partition) birleştirir:

- **OverlayFS** — geniş uyumluluk için katmanlı bağlamalar.
- **Magic Mount** — doğrudan yol değiştirme veya yedek çözüm için bind-mount.
- **Kasumi** — çalışma zamanı gizleme, sahteleme (spoof) ve gizlenme (stealth) özellikleriyle LKM destekli yönlendirme.

Yerleşik bir **SolidJS WebUI**, grafiksel yönetim, canlı durum izleme ve yapılandırma düzenleme imkânı sağlar.

Sürümler üç farklı türde yayınlanır — ayrıntılı bir karşılaştırma için [Derleme Türleri](#derleme-türleri) bölümüne bakın. Aksi belirtilmedikçe, bu README'nin geri kalanı `full` derlemesini anlatmaktadır.

**[English](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/README.md)** &nbsp; **[简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH.md)** &nbsp; **[繁體中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH_TW.md)** &nbsp; **[日本語](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_JP.md)** &nbsp; **[Español](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ES.md)** &nbsp; **[Italiano](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_IT.md)** &nbsp; **[Русский](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_RU.md)** &nbsp; **[Українська](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_UK.md)** &nbsp; **[Tiếng Việt](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_VI.md)** &nbsp; **[Türkçe](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_TR.md)**

---

## İçindekiler

- [Özellikler](#özellikler)
- [Derleme Türleri](#derleme-türleri)
- [Hızlı Başlangıç](#hızlı-başlangıç)
- [Bağlama Modları](#bağlama-modları)
- [WebUI](#webui)
- [Dil Desteği](#dil-desteği)
- [Yapılandırma](#yapılandırma)
- [Kasumi](#kasumi)
- [Politika Referansı](#politika-referansı)
- [CLI](#cli)
- [Mimari](#mimari)
- [Derleme](#derleme)
- [Operasyonel Notlar](#operasyonel-notlar)
- [Lisans](#lisans)

---

## Derleme Türleri

Hybrid Mount, her biri farklı bir kullanım senaryosunu hedefleyen üç farklı türde yayınlanır:

| Tür | İkili dosya (Binary) | WebUI | Daemon / CLI | Kasumi LKM | Kullanım senaryosu |
|--------|--------|-------|-------------|------------|----------|
| **Full** | Evet | Evet | Evet | Evet | Kasumi destekli yönlendirmeye veya gizleme/sahteleme yeteneklerine ihtiyaç duyan kullanıcılar. |
| **Lite** | Evet | Evet | Evet | Hayır | WebUI ve tam politika motorunu isteyen ancak LKM destekli gizlenme (stealth) özelliklerine ihtiyaç duymayan kullanıcılar. |
| **Nano** | Evet | Hayır | Hayır | Hayır | Sadece yapılandırma dosyası üzerinden bağlama orkestrasyonu isteyen minimalistler — çalışma zamanı daemon'u yok, WebUI yok, CLI yok. |

### Full

`full` türü, desteklenen tüm bağlama arka uçlarını (OverlayFS, Magic Mount, Kasumi), SolidJS WebUI'yi, HTTP/SSE destekli Unix-socket daemon'unu, CLI'yi ve Kasumi LKM dosyalarını içerir. Kasumi destekli yönlendirme veya yardımcı gizleme/sahteleme özellikleri gerektiğinde Full'u kullanın. `kasumi` Cargo özelliğiyle (bu özellik `control-plane`'i de otomatik olarak etkinleştirir) derlenir.

### Lite

`lite` türü, Kasumi LKM'sini ve Kasumi ile ilgili tüm özellikleri (gizleme, sahteleme, gizlenme (stealth), kstat kuralları, uname sahteciliği vb.) kaldırır; ancak WebUI, daemon, CLI ile hem OverlayFS hem de Magic Mount arka uçlarını korur. Aşağıdaki durumlarda Lite'ı tercih edin:

- Çekirdeğiniz harici LKM yüklemeyi desteklemiyorsa.
- Çalışma zamanı gizleme/sahteleme yeteneklerine ihtiyacınız yoksa.
- WebUI ve daemon yönetim arayüzünü korurken daha küçük bir indirme boyutu istiyorsanız.

Lite derlemeleri yalnızca `control-plane` özellik setini kullanır (`--no-default-features --features control-plane`). WebUI'deki Kasumi paneli otomatik olarak gizlenir.

### Nano

`nano` türü, **yalnızca yapılandırma** ile çalışan bir derlemedir (`--no-default-features` — hiçbir Cargo özelliği etkin değildir). WebUI'yi, daemon'u, CLI'yi ve tüm control-plane altyapısını kaldırır. Geriye, `config.toml` dosyasını okuyan, bir bağlama planı oluşturan ve bunu uygulayan, ardından sonlanan minimal bir ikili dosya kalır. Temel özellikler:

- **Çalışma zamanı daemon'u yok** — arka plan işlemi, socket, WebUI veya CLI alt komutu bulunmaz.
- **WebUI yok** — `webroot/`, `launcher.png` ve `service.sh` dosyaları paketten çıkarılır.
- **Yalnızca bağlama işlemi** — ikili dosya önyükleme sırasında çalışır, yapılandırmaya göre her şeyi bağlar ve sonlanır.
- **Varsayılan mod `magic`'tir** — Nano, ext4 imajlarını yönetecek bir daemon bulunmadığında bind mount'ları tercih ederek yapılandırmasında önceden `default_mode = "magic"` ayarıyla gelir.
- **Modül modu işaretçileri** — kurulum sırasında ses tuşlarıyla yapılan seçim, yönetilen her modül kök dizinine boş bir `overlay` veya `magic` işaretçisi yazar; Nano bir beyaz liste yerine bunu okur. İşaretçi dosya adları büyük/küçük harfe duyarsız şekilde eşleştirilir.
- **Yerleşik Hybrid Mount süreci yoktur** — önyükleme sırasındaki bağlama işlemi tamamlandıktan sonra Nano ikili dosyası sonlanır.

Öngörülebilir, daemon'suz ve daha küçük bir çalışma zamanı yüzeyine sahip bağlama orkestrasyonu istiyorsanız Nano'yu tercih edin.

### Özellik Matrisi

| Özellik | Full | Lite | Nano |
|---------|------|------|------|
| OverlayFS arka ucu | Evet | Evet | İşaretçi tabanlı |
| Magic Mount arka ucu | Evet | Evet | Evet (varsayılan) |
| Kasumi arka ucu | Evet | Hayır | Hayır |
| WebUI | Evet | Evet | Hayır |
| CLI (`hybrid-mount` alt komutları) | Evet | Evet | Hayır |
| Daemon (Unix + TCP/SSE) | Evet | Evet | Hayır |
| Yapılandırma önbellekleme ve çalışma zamanı uygulama | Evet | Evet | Hayır |
| Kasumi gizleme/sahteleme/gizlenme (stealth) | Evet | Hayır | Hayır |
| LKM otomatik yükleme | Evet | Hayır | Hayır |
| Cargo özellikleri | `kasumi` (`control-plane`'i de otomatik etkinleştirir) | yalnızca `control-plane` | yok |
| ZIP boyutu (yaklaşık) | ~4 MB | ~2 MB | ~1 MB |

## Özellikler

- **Üç arka uç, tek bir politika motoru** — yollara (path) OverlayFS, Magic Mount veya Kasumi'yi yol bazında ayrıntılı şekilde atayın.
- **Belirlenimci (deterministic) planlama** — çakışmalar önyükleme sırasında rastgele fark edilmek yerine planlama aşamasında tespit edilir.
- **Yerleşik WebUI** — modülleri yönetin, yapılandırmayı düzenleyin, çalışma zamanı durumunu izleyin ve full derlemelerinde Kasumi özelliklerini kontrol edin.
- **Kasumi çalışma zamanı entegrasyonu** — LKM otomatik yükleme, mirror yönlendirmesi, bağlama gizleme, maps/statfs sahteciliği, UID gizleme, uname sahteciliği ve kstat kuralları.
- **Yapılandırma önbellekleme** — artımlı yama (patch) uygulama ve anında etkinleştirme desteğine sahip çalışma zamanı yapılandırma önbelleği.
- **Kurtarmaya uygun** — eskimiş çalışma zamanı dosyaları otomatik olarak temizlenir; hatalı yapılandırmalar `api config-reset` ile sıfırlanabilir.
- **Otomasyona uygun** — betik yazımı veya harici denetleyiciler için JSON-over-Unix-socket daemon protokolü ve HTTP API.

---

## Hızlı Başlangıç

### Kurulum

1. Cihazınıza [KernelSU](https://kernelsu.org/) veya [APatch](https://apatch.dev/) kurun.
2. [GitHub Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases) sayfasından en son Hybrid Mount `full`, `lite` veya `nano` sürüm ZIP dosyasını indirin.
3. ZIP dosyasını root yöneticinizin modül yükleyicisi üzerinden flashleyin.
4. Yeniden başlatın. Hybrid Mount ortamınızı otomatik olarak algılayacak ve varsayılan overlay politikasını uygulayacaktır.

### Kurulum Sonrası

```bash
# Çalışma zamanı durumunu kontrol et
hybrid-mount daemon status

# Algılanan modülleri listele
hybrid-mount api modules-list
```

WebUI'ye erişmek için (Full/Lite türlerinde), root yönetici uygulamanızı (KernelSU veya APatch) açın, modül listesinde Hybrid Mount'u bulun ve üzerine dokunun — yönetici, WebUI'yi gömülü bir WebView içinde başlatacaktır.

### Bir modül için bağlama modunu değiştirme

```toml
# /data/adb/hybrid-mount/config.toml
[rules.my_module]
default_mode = "magic"

[rules.my_module.paths]
"system/bin/problematic_binary" = "ignore"
```

---

## Bağlama Modları

| Mod | Arka Uç | En uygun kullanım |
|------|---------|----------|
| `overlay` | OverlayFS | Çakışma olmadan dosya ekleyen veya değiştiren modüller. Varsayılan mod. |
| `magic` | Bind mount | Doğrudan dosya bazında değiştirme gerektiren modüller. |
| `kasumi` | Kasumi LKM | Açık mirror yönlendirmesi veya çalışma zamanı gizleme/sahteleme özellikleri gerektiren modüller. |
| `ignore` | — | Belirli yolları herhangi bir bağlama işleminden hariç tutmak. |

### OverlayFS depolama modları

OverlayFS arka ucu, üst/çalışma (upper/work) katmanları için iki depolama stratejisini destekler:

- `ext4` (varsayılan) — bir ext4 disk imajı oluşturur. Yeniden başlatmalar arasında kalıcıdır, xattr'ı destekler.
- `tmpfs` — bir tmpfs bağlaması kullanır. Geçicidir, daha hafiftir, ancak yeniden başlatıldığında kaybolur.

```toml
overlay_mode = "ext4"
```

---

## WebUI

Hybrid Mount, daemon tarafından yerel bir TCP soketi (HTTP/SSE) üzerinden sunulan **SolidJS tabanlı bir WebUI** içerir. CLI ve otomasyon istemcileri bir Unix soketi üzerinden iletişim kurar. Daemon, başlangıçta WebUI erişim URL'sini logcat'e yazdırır.

WebUI, doğrudan **root yönetici uygulamanızdan** (KernelSU veya APatch yöneticisi) açılacak şekilde tasarlanmıştır — modül girişine dokunun, yönetici WebUI'yi gömülü bir WebView içinde başlatacaktır. Cihaz üzerinde harici bir tarayıcıya gerek yoktur.

### Yetenekler

- **Durum panosu** — canlı bağlama istatistikleri, etkin bölümler, depolama modu, daemon sağlığı.
- **Modül yönetimi** — algılanan tüm modülleri etkin bağlama modlarıyla listeleyin; mod değişikliklerini etkileşimli olarak uygulayın.
- **Yapılandırma düzenleyici** — modül başına yol kuralları dahil, doğrulamalı tam config.toml düzenleme.
- **Kasumi kontrol paneli** — LKM durumu, kural listeleme, özellik anahtarları, uname yapılandırması, maps/kstat kuralları (yalnızca Full türünde).

### Dil Desteği

WebUI şu anda aşağıdaki dillerle birlikte gelmektedir:

- İngilizce (`en-US`, varsayılan)
- İspanyolca (`es-ES`)
- İtalyanca (`it-IT`)
- Japonca (`ja-JP`)
- Rusça (`ru-RU`)
- Ukraynaca (`uk-UA`)
- Vietnamca (`vi-VN`)
- Basitleştirilmiş Çince (`zh-CN`)
- Geleneksel Çince (`zh-TW`)
- Türkçe (`tr-TR`)

README belgeleri [İngilizce](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/README.md), [Basitleştirilmiş Çince](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH.md), [Geleneksel Çince](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH_TW.md), [Japonca](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_JP.md), [İspanyolca](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ES.md), [İtalyanca](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_IT.md), [Rusça](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_RU.md), [Ukraynaca](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_UK.md), [Vietnamca](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_VI.md) ve [Türkçe](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_TR.md) dillerinde mevcuttur.

### Erişim

WebUI, kriptografik bir erişim token'ıyla `http://127.0.0.1:<random-port>` üzerinde çalışır. Yaşam döngüsünü daemon yönetir — ayrı bir web sunucusuna gerek yoktur. Cihaz üzerinde root yöneticinizin WebView'i üzerinden açın; uzaktan erişim için portu ADB ile yönlendirin.

---

## Yapılandırma

Varsayılan yol: `/data/adb/hybrid-mount/config.toml`.

### Üst düzey alanlar

| Anahtar | Tip | Varsayılan | Açıklama |
| --- | --- | --- | --- |
| `moduledir` | string | `/data/adb/modules` | Modül kaynak dizini. |
| `mountsource` | string | otomatik algılama | Çalışma zamanı kaynak etiketi (`KSU`, `APatch`). |
| `overlay_mode` | `ext4` \| `tmpfs` | `ext4` | Overlay üst/çalışma depolama modu. |
| `disable_umount` | bool | `false` | Umount işlemlerini atla (yalnızca hata ayıklama için). |
| `default_mode` | `overlay` \| `magic` \| `kasumi` | `overlay` | Genel varsayılan bağlama politikası. |
| `daemon_startup_mode` | `on-demand` \| `persistent` | `on-demand` | Daemon başlangıç davranışı. |
| `rules` | map | `{}` | Modül ve yol bazlı bağlama politikaları. |

### Örnek

```toml
moduledir = "/data/adb/modules"
overlay_mode = "ext4"
default_mode = "overlay"
daemon_startup_mode = "on-demand"

[rules.viper4android]
default_mode = "magic"

[rules.viper4android.paths]
"system/etc/audio_policy.conf" = "overlay"

[rules.sensitive_module]
default_mode = "kasumi"

[rules.sensitive_module.paths]
"system/bin/helper" = "kasumi"
"system/etc/placeholder" = "ignore"
```

---

## Kasumi

Kasumi, **LKM destekli** bir arka uçtur. Bağlama yönlendirmesinin ötesinde, bir dizi çalışma zamanı gizleme ve sahteleme yeteneği sunar.

### Etkinleştirme

`kasumi.enabled = true` ayarı, bu arka ucu kullanılabilir hale getirir. Kasumi çalışma zamanı, aşağıdaki koşullardan en az biri sağlandığında fiilen etkinleşir:

- Bağlama planı, Kasumi tarafından yönetilen bir modül veya yol içeriyorsa.
- Yardımcı bir özellik yapılandırılmışsa (hidexattr, bağlama gizleme, maps sahteciliği, statfs sahteciliği, UID gizleme, uname sahteciliği, cmdline değiştirme, kstat kuralları veya kullanıcı gizleme kuralları).

### Temel yapılandırma alanları

| Alan | Amaç |
| --- | --- |
| `kasumi.enabled` | Kasumi entegrasyonu için ana anahtar. |
| `kasumi.lkm_autoload` | Başlangıçta Kasumi LKM'sini otomatik yükler. |
| `kasumi.lkm_dir` | LKM arama dizini. |
| `kasumi.lkm_kmi_override` | LKM seçimi için isteğe bağlı KMI sürümü geçersiz kılma. |
| `kasumi.mirror_path` | Kasumi kuralları tarafından kullanılan mirror kök dizini (varsayılan `/dev/kasumi_mirror`). |
| `kasumi.enable_kernel_debug` | Çekirdek tarafı hata ayıklama günlüklemesini açar/kapatır. |
| `kasumi.enable_stealth` | Açık gizlenme (stealth) modu. |
| `kasumi.enable_hidexattr` | Uyumluluk şemsiyesi — gizlenme (stealth), bağlama gizleme, maps sahteciliği ve statfs sahteciliğini birlikte etkinleştirir. |
| `kasumi.enable_mount_hide` | Bağlamaları genel olarak veya yol desenine göre gizler. |
| `kasumi.mount_hide.path_pattern` | Bağlama gizleme için yol deseni. |
| `kasumi.enable_maps_spoof` | `/proc/<pid>/maps` sahteciliğini etkinleştirir. |
| `kasumi.maps_rules` | Inode/aygıt bazlı maps yeniden yazma kuralları. |
| `kasumi.enable_statfs_spoof` | `statfs` sahteciliğini etkinleştirir. |
| `kasumi.statfs_spoof.path` / `.spoof_f_type` | Yol kapsamlı statfs sahteciliği yapılandırması. |
| `kasumi.hide_uids` | Kasumi'ye duyarlı sorgulardan gizlenecek UID'ler. |
| `kasumi.uname_mode` | Uname sahteciliği modu: `scoped` (süreç bazlı) veya `global`. |
| `kasumi.uname.*` | Yapılandırılmış uname sahteciliği (sysname, nodename, release, version, machine, domainname). |
| `kasumi.cmdline_value` | Yerine geçecek `/proc/cmdline` içeriği. |
| `kasumi.kstat_rules` | Hedef bazlı stat meta verisi sahteciliği kuralları. |

### Komutlar

```bash
# Durum ve tanılama
hybrid-mount kasumi status
hybrid-mount kasumi version
hybrid-mount kasumi features
hybrid-mount kasumi hooks
hybrid-mount kasumi list          # etkin kuralları listele
hybrid-mount lkm status

# Çalışma zamanı kontrolü
hybrid-mount kasumi apply-config-runtime
hybrid-mount kasumi clear
hybrid-mount kasumi release-connection
hybrid-mount kasumi invalidate-cache
hybrid-mount kasumi fix-mounts

# Uname sahteciliği (scoped veya global)
hybrid-mount kasumi set-uname --mode scoped <release> <version>
hybrid-mount kasumi clear-uname --mode scoped
hybrid-mount kasumi restore-uname-global

# Kural yönetimi
hybrid-mount kasumi rule add --target /system/bin/tool --source /data/adb/modules/my_module/system/bin/tool
hybrid-mount kasumi rule merge --target /system/lib64 --source /data/adb/modules/my_module/system/lib64
hybrid-mount kasumi rule hide --path /system/bin/su
hybrid-mount kasumi rule delete --path /system/bin/old_tool
hybrid-mount kasumi rule add-dir --target-base /system/lib64 --source-dir /data/adb/modules/my_module/system/lib64
hybrid-mount kasumi rule remove-dir --target-base /system/lib64 --source-dir /data/adb/modules/my_module/system/lib64
```

---

## Politika Referansı

### Öncelik

Bir yola birden fazla politika uygulanabildiğinde, değerlendirme sırası şu şekildedir:

1. **Yol düzeyinde geçersiz kılma** — `rules.<module>.paths["<path>"]`
2. **Modül düzeyinde varsayılan** — `rules.<module>.default_mode`
3. **Genel varsayılan** — `default_mode`

### Davranış matrisi

| Kural sonucu | Arka uç mevcut mu? | Etkin davranış |
| --- | --- | --- |
| `overlay` | Evet | OverlayFS ile bağla. |
| `overlay` | Hayır | Atla ve başarısız olarak raporla. |
| `magic` | Uygulanamaz | Magic Mount ile bağla. |
| `kasumi` | Evet | Kasumi üzerinden yönlendir. |
| `kasumi` | Hayır | Kasumi eşlemesini atla. |
| `ignore` | Uygulanamaz | Hiçbir şey bağlama. |

### Modül işaretçi dosyaları

Hybrid Mount, modül dizinlerindeki işaretçi dosyalarını da tanır. Bu işaretçilerin normal dosyalar olması beklenir; yalnızca dosya adı kullanılır. İşaretçi dosya adları, ASCII harfleri için büyük/küçük harfe duyarsız şekilde eşleştirilir; bu nedenle `DISABLE`, `Disable` ve `disable` aynı işaretçi olarak kabul edilir.

| İşaretçi | Konum | Etki |
| --- | --- | --- |
| `disable` | Modül kök dizini | Modülü bağlama planlamasından hariç tutar ve devre dışı olarak raporlar. |
| `remove` | Modül kök dizini | Modülü bağlama planlamasından hariç tutar; normalde kaldırma sırasında root yöneticisi tarafından oluşturulur. |
| `skip_mount` | Modül kök dizini | Modülü bağlama işleminden hariç tutar ve çalışma zamanı atlama listesine kaydeder. |
| `mount_error` | Modül kök dizini | Bir bağlama hatası sonrasında atlanan modülü işaretler. Kurtarma ve daemon komutları bunu oluşturabilir veya temizleyebilir. |
| `overlay` / `magic` | Modül kök dizini, Nano derlemeleri | Nano derlemeleri için modülün varsayılan bağlama arka ucunu seçer. Full ve Lite derlemeleri bunun yerine yapılandırma kurallarını kullanır. |
| `.replace` | Modül dizini içinde | İçinde bulunduğu dizine değiştirme (replacement) semantiği uygular. İşaretçinin kendisi normal modül içeriği olarak kopyalanmaz; hazırlanan overlay katmanları dizini korur ve desteklendiği yerlerde overlay opak meta verisini ayarlar. |

Aynı işaretçinin birden fazla büyük/küçük harf varyantı bir dizinde bulunuyorsa, temizleme işlemleri eşleşen tüm varyantları kaldırır.

### Pratik örnekler

- **Bir sorunlu ikili dosya bind mount'ta, geri kalanı overlay'de**: modül varsayılanını `overlay` olarak ayarlayın, ikili dosya yolunu `magic` ile geçersiz kılın.
- **Çakışan bir dosyayı geçici olarak hariç tutmak**: yolu `ignore` olarak ayarlayın.

---

## CLI

```bash
hybrid-mount [OPTIONS] [COMMAND]
```

### Genel seçenekler

| Bayrak | Açıklama |
| ---- | ----------- |
| `-c, --config <PATH>` | Özel yapılandırma dosyası yolu. |

### Alt komutlar

| Komut | Açıklama |
| ------- | ----------- |
| `gen-config` | Varsayılan bir yapılandırma dosyası oluşturur. |
| `logs` | Son daemon günlüklerini yazdırır. |
| `api storage` | Depolama modunu sorgular (ext4/tmpfs). |
| `api mount-stats` | Bağlama istatistiklerini yazdırır. |
| `api mount-topology` | Bağlama topoloji ağacını yazdırır. |
| `api partitions` | Yönetilen bölümleri listeler. |
| `api system-info` | Sistem bilgilerini yazdırır. |
| `api version` | Daemon sürümünü yazdırır. |
| `api config-get` | Etkin yapılandırmayı JSON olarak yazdırır. |
| `api config-set --config <JSON>` | Tüm yapılandırmayı değiştirir. |
| `api config-patch --patch <JSON>` | Yapılandırmaya bir yama (patch) birleştirir. |
| `api config-reset` | Yapılandırmayı varsayılanlara sıfırlar. |
| `api modules-list` | Algılanan modülleri listeler. |
| `api modules-apply --modules <JSON>` | Modül modu değişikliklerini uygular. |
| `api lkm` | LKM durumunu sorgular. |
| `api features` | Desteklenen özellikleri listeler. |
| `api hooks` | Kasumi hook durumlarını listeler. |
| `api kernel-uname` | Çekirdek uname bilgisini yazdırır. |
| `api open-url --url <URL>` | Cihazda bir URL açar. |
| `api reboot` | Cihazı yeniden başlatır. |
| `api kasumi-maps-add --rule <JSON>` | Bir Kasumi maps sahteciliği kuralı ekler. |
| `api kasumi-maps-clear` | Tüm Kasumi maps sahteciliği kurallarını temizler. |
| `daemon launch` | Daemon'u ön planda başlatır. |
| `daemon serve` | Daemon'u başlatır (servis modu). |
| `daemon ping` | Daemon'un çalışır durumda olup olmadığını kontrol eder. |
| `daemon webui-start` | Yalnızca WebUI'yi başlatır. |
| `daemon stop` | Daemon'u durdurur. |
| `daemon status` | Daemon'un çalışma zamanı durumunu sorgular. |
| `kasumi ...` | Kasumi yönetimi (bkz. [Kasumi](#kasumi)). |
| `lkm load / unload / status` | LKM yaşam döngüsü yönetimi. |
| `hide list / add / remove / apply` | Kullanıcı gizleme kuralı yönetimi. |

---

## Mimari

```text
┌─────────────────────────────────────────────┐
│                  config.toml                  │
└──────────────────┬──────────────────────────┘
                   ▼
┌─────────────────────────────────────────────┐
│                 Envanter Keşfi                │
│  Modül ağacını tara, girdileri sınıflandır   │
└──────────────────┬──────────────────────────┘
                   ▼
┌─────────────────────────────────────────────┐
│              Bağlama Planlayıcı               │
│  Kuralları değerlendir (yol > modül > genel)  │
│     overlay / magic / kasumi planı oluştur    │
└──────────────────┬──────────────────────────┘
                   ▼
┌─────────────────────────────────────────────┐
│                  Yürütücüler                  │
│  ┌──────────┐ ┌──────────┐ ┌──────────────┐ │
│  │ OverlayFS│ │  Magic   │ │   Kasumi     │ │
│  │ yürütücü │ │  Mount   │ │   yürütücü   │ │
│  └──────────┘ └──────────┘ └──────────────┘ │
└──────────────────┬──────────────────────────┘
                   ▼
┌─────────────────────────────────────────────┐
│        Çalışma Zamanı Durumu + Daemon         │
│ Durumu kalıcılaştır → Unix socket → WebUI/CLI │
└─────────────────────────────────────────────┘
```

Yürütücü, bir **typestate durum makinesi** (`src/core/controller.rs`) tarafından yönetilir: `MountController<Init> → StorageReady → Planned → Executed`. Her geçiş bir işlem hattı aşamasını temsil eder ve bağlama işleminin her zaman iyi tanımlanmış bir durumda olmasını sağlar.

### Kaynak kod düzeni

```text
src/
├── conf/          Yapılandırma şeması, TOML yükleyici, CLI tanımı, işleyiciler
├── domain/        Temel tipler: MountMode, ModuleRules, yol eşleştirme
├── partitions/    Yönetilen bölümlerin otomatik keşfi
├── core/
│   ├── inventory/ Modül keşfi ve listeleme
│   ├── ops/       Bağlama planı oluşturma ve arka uç bazlı yürütme
│   ├── daemon/    Unix + TCP çift protokollü daemon (CLI + WebUI/SSE)
│   ├── api/       WebUI uç noktaları için payload oluşturucular
│   ├── startup/   Önyükleme sırası, kurtarma, yeniden deneme mantığı
│   ├── storage/   Paylaşılan depolama yardımcıları (ext4 imajı, tmpfs)
│   └── runtime_state/ Daemon durum kalıcılığı
├── mount/
│   ├── overlayfs/ OverlayFS arka ucu (ext4 imajı / tmpfs)
│   ├── magic_mount/ Bind-mount arka ucu
│   └── kasumi/    Kasumi kural derlemesi, çalışma zamanı, durum
├── sys/           Düşük seviye: mount syscall'ları, LKM yükleme/kaldırma, Kasumi UAPI
└── utils/         Günlükleme, yol yardımcıları, doğrulama

webui/
├── src/
│   ├── routes/    Sayfa bileşenleri (Status, Config, Modules, Kasumi, Info)
│   ├── components/ Paylaşılan UI bileşenleri (NavBar, Toast, Skeleton)
│   ├── lib/       API köprüsü, store'lar, codec'ler, i18n
│   └── locales/   9 dilli uluslararasılaştırma

xtask/             Derleme ve yayın otomasyonu
module/            Modül paketleme betikleri ve statik varlıklar
```

---

## Derleme

### Ön koşullar

- Rust nightly (`rust-toolchain.toml` dosyasından)
- Android NDK r27+ ve `cargo-ndk`
- Node.js 20+ ve pnpm (WebUI için)

### Komutlar

```bash
# Tam (full) yayın paketi (binary + WebUI + Kasumi) → output/
cargo run -p xtask -- build --release --flavor full

# Lite yayın paketi (binary + WebUI, Kasumi yok) → output/
cargo run -p xtask -- build --release --flavor lite

# Nano yayın paketi (yalnızca yapılandırma, WebUI/CLI/daemon yok) → output/
cargo run -p xtask -- build --release --flavor nano

# Yalnızca ikili dosya (WebUI'yi atla)
cargo run -p xtask -- build --release --skip-webui

# Yerel arm64 hata ayıklama derlemesi
./scripts/build-local.sh

# Yerel lite hata ayıklama derlemesi
./scripts/build-local.sh --lite

# Yerel nano hata ayıklama derlemesi
./scripts/build-local.sh --nano

# Önceden derlenmiş Kasumi LKM .ko dosyalarıyla yerel derleme (yalnızca full)
./scripts/build-local.sh --release --kasumi-lkm-dir /path/to/kasumi-lkm

# WebUI geliştirme sunucusu (hot reload)
cd webui && pnpm install && pnpm dev

# Her şeyi lint'le
cargo run -p xtask -- lint
cd webui && pnpm lint

# Testleri çalıştır
cargo +nightly test
cd webui && pnpm test
```

### Yayın profili

Yayın profili, ikili dosya boyutunu küçültmek için `opt-level = 3`, `lto = "fat"`, `codegen-units = 1`, `strip = true` ve `panic = "abort"` ayarlarını kullanır.

### CI kontrolleri ve özellik bayrağı denetimi

Her değişiklik, aşağıdaki CI kontrollerinden geçmelidir (`.github/workflows/` içinde tanımlıdır):

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings` (uyarılar hata olarak kabul edilir)
- `cargo test --all-targets --workspace`
- WebUI: `pnpm lint` + `pnpm test`
- Tüm kaynak dosyalarında lisans başlığı kontrolü

`cargo clippy --all-features` (`xtask lint` tarafından çalıştırılır) yalnızca `full` türünü kontrol eder. Değişiklik yaparken, **lite** (`--no-default-features --features control-plane`) ve **nano** (`--no-default-features`) tür kombinasyonlarının da derlendiğini doğrulayın. Kasumi ile ilgili kod `#[cfg(feature = "kasumi")]` arkasında olmalıdır; daemon/CLI/WebUI API'sine dokunan kod ise `#[cfg(feature = "control-plane")]` arkasında olmalıdır.

---

## Operasyonel Notlar

- **Bağlama kaynağı otomatik algılama**: yeni kurulumlar çalışma zamanı ortamını otomatik olarak algılar. `mountsource`'u yalnızca otomatik algılama başarısız olursa açıkça ayarlayın.
- **Hatalı yapılandırmadan kurtarma**: varsayılanlara sıfırlamak için `hybrid-mount api config-reset` komutunu çalıştırın, ardından kuralları kademeli olarak yeniden uygulayın. Yeni bir yapılandırma dosyası oluşturmak için `gen-config` kullanın.
- **Yapılandırma önbellekleme**: çalışma zamanı önbelleğe alınmış bir yapılandırma tutar. Değişiklikleri hemen uygulamak için `api config-patch --apply-runtime` kullanın veya daemon'u yeniden başlatın.
- **Kasumi LKM (yalnızca full derlemelerde)**: LKM, çalışan çekirdekle eşleşmelidir. Otomatik algılanan KMI yanlışsa `lkm_kmi_override` kullanın.
- **`kasumi clear`**: çalışma zamanı durumunu temizler ve çekirdek bağlantısını serbest bırakır. Mevcut çekirdek tarafı kurallar, LKM yeniden yüklenene kadar kalıcı olabilir.
- **İkili dosya boyutu**: köklü yeniden yapılandırmadan önce bağımlılık özelliği kırpma ve profil ayarlamasını tercih edin.

---

## Lisans

[Apache-2.0](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/LICENSE) lisansı altında lisanslanmıştır.
