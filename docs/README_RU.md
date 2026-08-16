# Hybrid Mount

<img src="https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/main/icon.svg" align="right" width="120" />

![Language](https://img.shields.io/badge/Language-Rust-orange?style=flat-square&logo=rust)
![Platform](https://img.shields.io/badge/Platform-Android-green?style=flat-square&logo=android)
![License](https://img.shields.io/badge/License-GPL--3.0-blue?style=flat-square)
![Version](https://img.shields.io/github/v/tag/Hybrid-Mount/meta-hybrid_mount?label=Version&color=8A2BE2&style=flat-square)

Hybrid Mount — метамодуль для оркестрации монтирования в **KernelSU** и **APatch**.
Он объединяет файлы модулей с разделами Android через единый движок политик и два backend-а монтирования:

- **OverlayFS**: слоистое монтирование для широкой совместимости.
- **Magic Mount**: bind mount для прямой замены путей или fallback.

Встроенная **WebUI на SolidJS** предоставляет графическое управление, мониторинг состояния и редактирование конфигурации.

Пакеты выпускаются в двух вариантах. Если не указано иное, этот README описывает вариант `lite`.

**[English](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/README.md)** &nbsp; **[简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH.md)** &nbsp; **[繁體中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH_TW.md)** &nbsp; **[日本語](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_JP.md)** &nbsp; **[Español](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ES.md)** &nbsp; **[Italiano](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_IT.md)** &nbsp; **[Русский](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_RU.md)** &nbsp; **[Українська](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_UK.md)** &nbsp; **[Tiếng Việt](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_VI.md)**

---

## Содержание

- [Возможности](#возможности)
- [Варианты сборки](#варианты-сборки)
- [Быстрый старт](#быстрый-старт)
- [Режимы монтирования](#режимы-монтирования)
- [WebUI](#webui)
- [Поддержка языков](#поддержка-языков)
- [Конфигурация](#конфигурация)
- [Справка по политикам](#справка-по-политикам)
- [CLI](#cli)
- [Архитектура](#архитектура)
- [Сборка](#сборка)
- [Эксплуатационные заметки](#эксплуатационные-заметки)
- [Лицензия](#лицензия)

---

## Варианты сборки

| Вариант | Бинарный файл | WebUI | Daemon / CLI | Сценарий |
|---------|---------------|-------|--------------|----------|
| **Lite** | Да | Да | Да | Для пользователей, которым нужны WebUI и полный движок политик без LKM-based stealth. |
| **Nano** | Да | Нет | Нет | Для конфигурационного монтирования без runtime-daemon, WebUI и CLI. |

### Lite

Вариант `lite` является вариантом по умолчанию. Он сохраняет WebUI, daemon, CLI, OverlayFS и Magic Mount. Он подходит, если ядро не поддерживает внешние LKM или runtime-возможности hide/spoof не требуются.

### Nano

Вариант `nano` (`--no-default-features`, без Cargo features) работает только через файл конфигурации. Он исключает WebUI, daemon, CLI и инфраструктуру control plane; остается небольшой бинарный файл, который читает `config.toml`, строит план монтирования, выполняет его и завершает работу.

Nano использует `magic` как режим по умолчанию. Во время установки выбор клавишами громкости создает пустые marker-файлы `overlay` или `magic` в корне управляемого модуля. Имена marker-файлов сравниваются без учета регистра.

### Матрица возможностей

| Возможность | Lite | Nano |
|-------------|------|------|
| Backend OverlayFS | Да | По marker-файлам |
| Backend Magic Mount | Да | Да, по умолчанию |
| WebUI | Да | Нет |
| CLI | Да | Нет |
| Daemon | Да | Нет |
| Кэш конфигурации и runtime-apply | Да | Нет |
| Cargo features | только `control-plane` | нет |
| Размер ZIP (прим.) | ~2 MB | ~1 MB |

## Возможности

- **Два backend-а, один движок политик**: назначение OverlayFS или Magic Mount на уровне отдельных путей.
- **Детерминированное планирование**: конфликты обнаруживаются на этапе построения плана.
- **Встроенная WebUI**: управление модулями, редактирование конфигурации и мониторинг runtime-состояния.
- **Кэш конфигурации**: инкрементальные patch-изменения и немедленное применение.
- **Восстановление**: автоматическая очистка устаревших runtime-файлов и сброс через `api config-reset`.
- **Автоматизация**: daemon protocol JSON-over-Unix-socket и HTTP API.

---

## Быстрый старт

1. Установите [KernelSU](https://kernelsu.org/) или [APatch](https://apatch.dev/) на устройство.
2. Скачайте ZIP `lite` или `nano` из [GitHub Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases).
3. Установите ZIP через установщик модулей root-менеджера.
4. Перезагрузите устройство. Hybrid Mount определит окружение и применит политику overlay по умолчанию.

```bash
# Проверить runtime-состояние
hybrid-mount daemon status

# Показать обнаруженные модули
hybrid-mount api modules-list
```

В варианте Lite WebUI открывается из записи модуля в KernelSU или APatch.

### Изменение режима монтирования модуля

```toml
# /data/adb/hybrid-mount/config.toml
[rules.my_module]
default_mode = "magic"

[rules.my_module.paths]
"system/bin/problematic_binary" = "ignore"
```

---

## Режимы монтирования

| Режим | Backend | Подходит для |
|-------|---------|--------------|
| `overlay` | OverlayFS | Модулей, добавляющих или заменяющих файлы без конфликтов. Режим по умолчанию. |
| `magic` | Bind mount | Прямой замены отдельных файлов. |
| `ignore` | Нет | Исключения конкретных путей из обработки монтирования. |

OverlayFS поддерживает `ext4` как постоянное хранилище по умолчанию и `tmpfs` как легкий временный вариант.
---

## WebUI

WebUI на SolidJS обслуживается daemon-ом через локальный TCP socket с HTTP/SSE. CLI и автоматизированные клиенты используют Unix socket.

Основные возможности:

- Панель состояния со статистикой, разделами, storage mode и состоянием daemon.
- Управление модулями и интерактивное изменение политик.
- Редактор `config.toml` с проверкой и правилами по путям.

### Поддержка языков

WebUI включает следующие locale:

- English (`en-US`, по умолчанию)
- Español (`es-ES`)
- Italiano (`it-IT`)
- 日本語 (`ja-JP`)
- Русский (`ru-RU`)
- Українська (`uk-UA`)
- Tiếng Việt (`vi-VN`)
- 简体中文 (`zh-CN`)
- 繁體中文 (`zh-TW`)

README-документация доступна на [English](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/README.md), [简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH.md), [繁體中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH_TW.md), [日本語](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_JP.md), [Español](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ES.md), [Italiano](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_IT.md), [Русский](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_RU.md), [Українська](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_UK.md) и [Tiếng Việt](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_VI.md).

---

## Конфигурация

Путь по умолчанию: `/data/adb/hybrid-mount/config.toml`.

| Поле | Тип | По умолчанию | Описание |
| --- | --- | --- | --- |
| `moduledir` | string | `/data/adb/modules` | Исходный каталог модулей. |
| `mountsource` | string | auto-detect | Runtime-окружение (`KSU`, `APatch`). |
| `overlay_mode` | `ext4` \| `tmpfs` | `ext4` | Хранилище upper/work для OverlayFS. |
| `disable_umount` | bool | `false` | Пропуск umount, только для отладки. |
| `default_mode` | `overlay` \| `magic` | `overlay` | Глобальная политика по умолчанию. |
| `daemon_startup_mode` | `on-demand` \| `persistent` | `on-demand` | Режим запуска daemon. |
| `rules` | map | `{}` | Политики по модулям и путям. |

---


## Справка по политикам

Порядок приоритета:

1. Переопределение по пути: `rules.<module>.paths["<path>"]`
2. Значение по умолчанию для модуля: `rules.<module>.default_mode`
3. Глобальное значение по умолчанию: `default_mode`

Распознаваемые marker-файлы: `disable`, `remove`, `skip_mount`, `mount_error`, `overlay`, `magic` и `.replace`. Имена marker-файлов сравниваются без учета регистра.

---

## CLI

```bash
hybrid-mount [OPTIONS] [COMMAND]
```

Частые подкоманды:

- `gen-config`: создать конфигурацию по умолчанию.
- `logs`: вывести последние логи daemon.
- `api config-get` / `api config-set` / `api config-patch` / `api config-reset`: управление конфигурацией.
- `api modules-list` / `api modules-apply`: просмотр и применение политик модулей.
- `daemon launch` / `daemon serve` / `daemon status` / `daemon stop`: управление daemon.

---

## Архитектура

Hybrid Mount читает `config.toml`, обнаруживает inventory модулей, строит план монтирования по правилам пути, модуля и глобальным правилам, затем выполняет его через OverlayFS или Magic Mount. Исполнитель управляется **типизированным конечным автоматом** (`src/core/controller.rs`): `MountController<Init> → StorageReady → Planned → Executed`. Каждый переход представляет один этап конвейера. Вариант Lite сохраняет runtime-состояние и предоставляет доступ к нему через WebUI и CLI.

Основные каталоги:

- `src/conf`: schema конфигурации, TOML loader, CLI и handlers.
- `src/domain`: основные типы, правила и matching путей.
- `src/core`: inventory, планирование, daemon, API, startup и runtime state.
- `src/mount`: backend-ы OverlayFS и Magic Mount.
- `src/sys`: mount syscalls.
- `webui`: SolidJS WebUI и i18n на 9 языках.
- `xtask`: автоматизация сборки и релиза.

---

## Сборка

Требования:

- Rust nightly из `rust-toolchain.toml`
- Android NDK r27+ и `cargo-ndk`
- Node.js 20+ и pnpm для WebUI

```bash
cargo run -p xtask -- build --release --flavor lite
cargo run -p xtask -- build --release --flavor nano
cargo run -p xtask -- build --release --skip-webui
./scripts/build-local.sh
cargo run -p xtask -- lint
cargo +nightly test
```

### CI-гейты и проверка feature flags

Каждое изменение должно проходить: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets --workspace`, WebUI `pnpm lint` + `pnpm test`, и проверку заголовка лицензии. Также убедитесь, что комбинации **lite** (`--no-default-features --features control-plane`) и **nano** (`--no-default-features`) компилируются. Код daemon/CLI/WebUI должен быть за `#[cfg(feature = "control-plane")]`.

---

## Эксплуатационные заметки

- Новые установки определяют `mountsource` автоматически.
- При поврежденной конфигурации используйте `hybrid-mount api config-reset`, затем применяйте правила постепенно.
- `api config-patch --apply-runtime` применяет частичные изменения сразу.

---

## Лицензия

Лицензировано под [GPL-3.0](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/LICENSE).
