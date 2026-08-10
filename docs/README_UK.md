# Hybrid Mount

<img src="https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/main/icon.svg" align="right" width="120" />

![Language](https://img.shields.io/badge/Language-Rust-orange?style=flat-square&logo=rust)
![Platform](https://img.shields.io/badge/Platform-Android-green?style=flat-square&logo=android)
![License](https://img.shields.io/badge/License-GPL--3.0-blue?style=flat-square)
![Version](https://img.shields.io/github/v/tag/Hybrid-Mount/meta-hybrid_mount?label=Version&color=8A2BE2&style=flat-square)

Hybrid Mount — це метамодуль оркестрації монтування для **KernelSU** та **APatch**.
Він поєднує файли модулів із розділами Android через єдиний рушій політик і два backend-и монтування:

- **OverlayFS**: шарове монтування для широкої сумісності.
- **Magic Mount**: bind mount для прямої заміни шляхів.

Вбудована **WebUI на SolidJS** забезпечує графічне керування, моніторинг стану та редагування конфігурації.

Пакети публікуються у двох варіантах. Якщо не зазначено інше, цей README описує варіант Lite (за замовчуванням).

**[English](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/README.md)** &nbsp; **[简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH.md)** &nbsp; **[繁體中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH_TW.md)** &nbsp; **[日本語](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_JP.md)** &nbsp; **[Español](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ES.md)** &nbsp; **[Italiano](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_IT.md)** &nbsp; **[Русский](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_RU.md)** &nbsp; **[Українська](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_UK.md)** &nbsp; **[Tiếng Việt](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_VI.md)**

---

## Зміст

- [Можливості](#можливості)
- [Варіанти збірки](#варіанти-збірки)
- [Швидкий старт](#швидкий-старт)
- [Режими монтування](#режими-монтування)
- [WebUI](#webui)
- [Підтримка мов](#підтримка-мов)
- [Конфігурація](#конфігурація)
- [Довідник політик](#довідник-політик)
- [CLI](#cli)
- [Архітектура](#архітектура)
- [Збірка](#збірка)
- [Ліцензія](#ліцензія)

---

## Варіанти збірки

Hybrid Mount випускається у двох варіантах, кожен під свій сценарій використання:

| Варіант | Бінарний файл | WebUI | Daemon / CLI | Сценарій використання |
|---------|---------------|-------|--------------|----------------------|
| **Lite (за замовчуванням)** | Так | Так | Так | Випуск за замовчуванням: WebUI, daemon, CLI та обидва backend-и: OverlayFS і Magic Mount. |
| **Nano** | Так | Ні | Ні | Для конфігураційного монтування без runtime-daemon, WebUI та CLI. |

### Lite

Lite — варіант за замовчуванням. Включає WebUI на SolidJS, daemon на Unix socket (HTTP/SSE), CLI та обидва backend-и: OverlayFS і Magic Mount:

- Потрібні WebUI та повний рушій політик.
- Потрібен менший пакет із збереженням WebUI та інтерфейсу керування daemon.

Збірки Lite використовують лише `control-plane` (`--no-default-features --features control-plane`).

### Nano

Варіант `nano` (`--no-default-features`, без Cargo features) працює лише через файл конфігурації. Він вилучає WebUI, daemon, CLI та інфраструктуру control plane; залишається невеликий бінарний файл, який читає `config.toml`, будує план монтування, виконує його й завершується.

Nano використовує `magic` як режим за замовчуванням. Під час встановлення вибір клавішами гучності створює порожні marker-файли `overlay` або `magic` у корені керованого модуля. Імена marker-файлів мають точно відповідати цим варіантам у нижньому регістрі.

### Матриця можливостей

| Можливість | Lite | Nano |
| ------------ | ------ | ------ |
| Backend OverlayFS | Так | Через marker-файли |
| Backend Magic Mount | Так | Так, за замовчуванням |
| WebUI | Так | Ні |
| CLI | Так | Ні |
| Daemon | Так | Ні |
| Runtime-застосування конфігурації | Так | Ні |
| Cargo features | тільки `control-plane` | немає |
| Розмір ZIP (прибл.) | ~2 MB | ~1 MB |

## Можливості

- **Детерміноване планування**: конфлікти виявляються під час побудови плану.
- **Вбудована WebUI**: керування модулями, редагування конфігурації та моніторинг runtime-стану.
- **Runtime-оновлення конфігурації**: перевірені patch-зміни зберігаються та застосовуються негайно.
- **Явні помилки**: недійсні стани й налаштування одразу завершуються помилкою; `api config-reset` викликається лише явно.
- **Автоматизація**: daemon protocol JSON-over-Unix-socket і HTTP API.

---

## Швидкий старт

1. Встановіть [KernelSU](https://kernelsu.org/) або [APatch](https://apatch.dev/) на пристрій.
2. Завантажте ZIP `lite` або `nano` з [GitHub Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases).
3. Встановіть ZIP через інсталятор модулів root-менеджера.
4. Під час першого встановлення виберіть типовий режим: збільшення гучності обирає OverlayFS, зменшення — Magic Mount, а через 10 секунд без вводу обирається OverlayFS. Це єдиний запит інсталятора; Nano пропускає цей крок.
5. Перезавантажте пристрій. Hybrid Mount визначить середовище й застосує вибрану політику.

```bash
# Перевірити runtime-стан
hybrid-mount daemon status

# Показати виявлені модулі
hybrid-mount api modules-list
```

У варіанті Lite WebUI відкривається із запису модуля в KernelSU або APatch.

### Зміна режиму монтування модуля

```toml
# /data/adb/hybrid-mount/config.toml
[rules.my_module]
default_mode = "magic"

[rules.my_module.paths]
"system/bin/problematic_binary" = "ignore"
```

---

## Режими монтування

| Режим | Backend | Найкраще для |
|-------|---------|--------------|
| `overlay` | OverlayFS | Модулів, що додають або замінюють файли без конфліктів. Режим за замовчуванням. |
| `magic` | Bind mount | Прямої заміни окремих файлів. |
| `ignore` | Немає | Виключення конкретних шляхів з обробки монтування. |

OverlayFS підтримує `ext4` як постійне сховище за замовчуванням і `tmpfs` як легкий тимчасовий варіант.
---

## WebUI

WebUI на SolidJS обслуговується daemon-ом через локальний TCP socket з HTTP/SSE. CLI й автоматизовані клієнти використовують Unix socket.

Основні можливості:

- Панель стану зі статистикою, розділами, storage mode і станом daemon.
- Керування модулями та інтерактивна зміна політик.
- Редактор `config.toml` з перевіркою та правилами за шляхами.

### Підтримка мов

WebUI містить такі locale:

- English (`en-US`, за замовчуванням)
- Español (`es-ES`)
- Italiano (`it-IT`)
- 日本語 (`ja-JP`)
- Русский (`ru-RU`)
- Українська (`uk-UA`)
- Tiếng Việt (`vi-VN`)
- 简体中文 (`zh-CN`)
- 繁體中文 (`zh-TW`)

README-документація доступна [English](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/README.md), [简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH.md), [繁體中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH_TW.md), [日本語](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_JP.md), [Español](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ES.md), [Italiano](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_IT.md), [Русский](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_RU.md), [Українська](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_UK.md) та [Tiếng Việt](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_VI.md).

---

## Конфігурація

Шлях за замовчуванням: `/data/adb/hybrid-mount/config.toml`.

| Поле | Тип | За замовчуванням | Опис |
| --- | --- | --- | --- |
| `moduledir` | string | `/data/adb/modules` | Початковий каталог модулів. |
| `mountsource` | string | auto-detect | Runtime-середовище (`KSU`, `APatch`). |
| `overlay_mode` | `ext4` \| `tmpfs` | `ext4` | Сховище upper/work для OverlayFS. |
| `disable_umount` | bool | `false` | Пропуск umount, лише для налагодження. |
| `rules` | map | `{}` | Політики за модулями та шляхами. |

---

## Довідник політик

Порядок пріоритету:

1. Перевизначення за шляхом: `rules.<module>.paths["<path>"]`
2. Значення модуля за замовчуванням: `rules.<module>.default_mode`
3. Глобальне значення за замовчуванням: `default_mode`

Розпізнавані marker-файли: `disable`, `remove`, `skip_mount`, `overlay`, `magic` і `.replace`. Регістр важливий, імена мають збігатися точно.

---

## CLI

```bash
hybrid-mount [OPTIONS] [COMMAND]
```

Поширені підкоманди:

- `gen-config`: створити конфігурацію за замовчуванням.
- `logs`: вивести останні логи daemon.
- `api config-get` / `api config-set` / `api config-patch` / `api config-reset`: керування конфігурацією.
- `api modules-list` / `api modules-apply`: перегляд і застосування політик модулів.
- `daemon launch` / `daemon serve` / `daemon status` / `daemon stop`: керування daemon.

---

## Архітектура

Основні каталоги:

- `src/conf`: schema конфігурації, TOML loader, CLI й handlers.
- `src/domain`: основні типи, правила та matching шляхів.
- `src/core`: inventory, планування, daemon, API, startup і runtime state.
- `webui`: SolidJS WebUI та i18n 9 мовами.
- `xtask`: автоматизація збірки й релізу.

---

## Збірка

Вимоги:

- Rust nightly з `rust-toolchain.toml`
- Android NDK r27+ і `cargo-ndk`
- Node.js 20+ і pnpm для WebUI

```bash
cargo run -p xtask -- build --release --flavor lite
cargo run -p xtask -- build --release --flavor nano
cargo run -p xtask -- build --release --skip-webui
./scripts/build-local.sh
cargo run -p xtask -- lint
cargo +nightly test
```

### CI-гейти та перевірка feature flags

---

## Ліцензія

Ліцензовано за [GPL-3.0](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/LICENSE).
