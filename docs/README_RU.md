# Hybrid Mount

Hybrid Mount — метамодуль гибридного монтирования для KernelSU и APatch. Во время загрузки он сканирует другие модули и для каждого элемента выбирает OverlayFS, Magic Mount или игнорирование в соответствии с глобальными правилами, правилами модуля и пути. Каталоги исходных файлов модулей всегда используются только для чтения.

## Возможности

- OverlayFS и Magic Mount можно сочетать на уровне отдельных модулей и путей.
- Правила пути имеют приоритет над настройками модуля по умолчанию, а настройки модуля — над глобальной настройкой по умолчанию.
- OverlayFS поддерживает режимы хранения tmpfs и ext4.
- Для подготовки ext4 KernelSU использует официальный ioctl, скрывающий узлы sysfs; APatch и другие среды без KSU по умолчанию используют встроенный совместимый LKM.
- Magic Mount поддерживает файлы, каталоги, символические ссылки, `.replace` и семантику whiteout.
- WebUI предоставляет интерфейсы MD3 (по умолчанию) и Miuix.
- Поддерживаются arm64, armv7 и x86_64; установщик автоматически выбирает подходящий бинарный файл.

## Установка

Скачайте ZIP со страницы [Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases) и установите его с помощью менеджера KernelSU или APatch. При первой установке выберите backend по умолчанию кнопками громкости. При обновлении файл `/data/adb/hybrid-mount/config.toml` сохраняется.

## Настройка

Конфигурация по умолчанию:

```toml
moduledir = "/data/adb/modules"
mountsource = "KSU"
overlay_mode = "ext4" # ext4 | tmpfs
disable_umount = false
default_mode = "overlay" # overlay | magic

[rules.example_module]
default_mode = "magic"

[rules.example_module.paths]
"system/etc/hosts" = "overlay"
```

Пути в правилах указываются относительно корня модуля. В правилах уровня модуля и пути также можно использовать `ignore`; глобальный backend по умолчанию принимает только `overlay` или `magic`. Один и тот же путь к файлу нельзя назначить обоим backend монтирования. Обычные каталоги могут использоваться обоими backend как общие структурные узлы, но конфликт файла, типа или `.replace` немедленно завершает этап планирования при загрузке с ошибкой. Изменения конфигурации вступают в силу после перезагрузки.

Эта маршрутизация не изменяет существующую проверку возможности `CONFIG_TMPFS_XATTR`. При установке в KernelSU весь каталог `lkm/` модуля удаляется, а во время работы используется только официальный ioctl `NukeExt4Sysfs`. При установке в APatch и другие среды без KSU каталог LKM сохраняется, и после монтирования подготовки ext4 система по умолчанию пытается его использовать. Встроенные файлы `.ko` поддерживают только aarch64. Для автоматического выбора требуется точное совпадение ветки ядра и метки Android/GKI; неизвестные комбинации отклоняются. Совместимость ABI готовых LKM всё равно необходимо проверять на соответствующем реальном устройстве. Если устройство аварийно завершит работу во время `insmod`, постоянный защитный маркер предотвратит повторную загрузку LKM при следующем запуске, сохранив остальные функции Hybrid Mount. Матрицу поддержки, контрольные суммы, исходный код и лицензии см. в [`module/lkm/README.md`](../module/lkm/README.md).

## Обратная связь

Перед установкой или отправкой сообщения о проблеме прочитайте [Уведомление об использовании](../USAGE_NOTICE.md). Приложите bugreport KernelSU/APatch, версию модуля и шаги для воспроизведения. Связаться с нами можно через [GitHub Issues](https://github.com/Hybrid-Mount/meta-hybrid_mount/issues) или [группу Telegram](https://t.me/hybridmountchat).

## Языки / Languages

- [English](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_EN.md)
- [Español](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_ES.md)
- [Français](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_FR.md)
- [Bahasa Indonesia](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_ID.md)
- [Italiano](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_IT.md)
- [日本語](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_JA.md)
- [Русский](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_RU.md)
- [Türkçe](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_TR.md)
- [Українська](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_UK.md)
- [Tiếng Việt](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_VI.md)
- [简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/README.md)
- [繁體中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_ZH_TW.md)

## Лицензия

- Ядро (Rust и скрипты модуля): GPL-3.0-only (см. [`LICENSE`](../LICENSE)).
- WebUI: Apache-2.0 (см. [`webui/LICENSE`](../webui/LICENSE)).
- Необязательный LKM для sysfs ext4 (исходный код и готовые файлы `.ko`): GPL-2.0-only, создан на основе [Mountify](https://github.com/backslashxx/mountify); см. [`module/lkm/README.md`](../module/lkm/README.md) и [`module/lkm/src/LICENSE`](../module/lkm/src/LICENSE).
