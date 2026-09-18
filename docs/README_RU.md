# Hybrid Mount

<img src="../icon.svg" alt="Hybrid Mount logo" align="right" width="120" />

Hybrid Mount — метамодуль гибридного монтирования для KernelSU и APatch. Во время загрузки он сканирует другие модули и для каждого элемента выбирает OverlayFS, Magic Mount, VFS или игнорирование в соответствии с глобальными правилами, правилами модуля и пути. Каталоги исходных файлов модулей всегда используются только для чтения.

## Возможности

- OverlayFS, Magic Mount и VFS можно сочетать на уровне отдельных модулей и путей.
- Правила пути имеют приоритет над настройками модуля по умолчанию, а настройки модуля — над глобальной настройкой по умолчанию.
- OverlayFS поддерживает режимы хранения tmpfs и ext4.
- Для подготовки ext4 KernelSU использует официальный ioctl, скрывающий узлы sysfs; APatch и другие среды без KSU по умолчанию используют встроенный совместимый LKM.
- Magic Mount поддерживает файлы, каталоги, символические ссылки, `.replace` и семантику whiteout.
- VFS передаёт правила внедрения в собственную подсистему VFS Hybrid Mount (модуль `hybridmount`) через keyring. Это независимая реализация, не совместимая с ядром NoMount и его CLI nm. В релизы входят исходники и предварительно собранный модуль arm64 для каждой поддерживаемой цели Android/GKI; он загружается при загрузке, если ядро его не содержит. При неудаче поведение определяется `vfs_strict`. VFS не является настоящим монтированием.
- WebUI предоставляет интерфейсы MD3 (по умолчанию) и Miuix.
- Поддерживаются arm64, armv7 и x86_64; установщик автоматически выбирает подходящий бинарный файл.

## Установка

Скачайте ZIP со страницы [Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases) и установите его с помощью менеджера KernelSU или APatch. При первой установке выберите backend по умолчанию кнопками громкости. При обновлении файл `/data/adb/hybrid-mount/config.toml` сохраняется.

## Настройка

Конфигурация по умолчанию:

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

Пути в правилах указываются относительно корня модуля. В правилах уровня модуля и пути также можно использовать `ignore`; глобальный backend по умолчанию принимает `overlay`, `magic` или `vfs`. VFS — это путь инъекции, а не реальное монтирование. Конфликт файла, типа или `.replace` немедленно завершает этап планирования при загрузке с ошибкой. Изменения конфигурации вступают в силу после перезагрузки.

Эта маршрутизация не изменяет существующую проверку возможности `CONFIG_TMPFS_XATTR`. При установке в KernelSU весь каталог `lkm/` модуля удаляется, а во время работы используется только официальный ioctl `NukeExt4Sysfs`. При установке в APatch и другие среды без KSU каталог LKM сохраняется, и после монтирования подготовки ext4 система по умолчанию пытается его использовать. Встроенные файлы `.ko` поддерживают только aarch64. Для автоматического выбора требуется точное совпадение ветки ядра и метки Android/GKI; неизвестные комбинации отклоняются. Совместимость ABI готовых LKM всё равно необходимо проверять на соответствующем реальном устройстве. Если устройство аварийно завершит работу во время `insmod`, постоянный защитный маркер предотвратит повторную загрузку LKM при следующем запуске, сохранив остальные функции Hybrid Mount. Матрицу поддержки, контрольные суммы, исходный код и лицензии см. в [`module/lkm/README.md`](../module/lkm/README.md).

## Обратная связь

Перед установкой или отправкой сообщения о проблеме прочитайте [Уведомление об использовании](../USAGE_NOTICE.md). Приложите bugreport KernelSU/APatch, версию модуля и шаги для воспроизведения. Связаться с нами можно через [GitHub Issues](https://github.com/Hybrid-Mount/meta-hybrid_mount/issues) или [группу Telegram](https://t.me/hybridmountchat).

## Языки / Languages

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

## Благодарности

- Спасибо [Anatdx](https://github.com/Anatdx)
- Спасибо [Tools-cx-app](https://github.com/Tools-cx-app)
- Спасибо [KernelSU](https://github.com/tiann/KernelSU)
- Спасибо [MKSU от 5ec1cff](https://github.com/5ec1cff/KernelSU)
- Спасибо [ReSukiSU](https://github.com/ReSukiSU/ReSukiSU)
- Спасибо [meta-magic_mount-rs](https://github.com/Tools-cx-app/meta-magic_mount-rs)
- Спасибо [NoMount](https://github.com/maxsteeel/nomount)

## Лицензия

- Ядро (Rust и скрипты модуля): GPL-3.0-only (см. [`LICENSE`](../LICENSE)).
- WebUI: Apache-2.0 (см. [`webui/LICENSE`](../webui/LICENSE)).
- Необязательный LKM для sysfs ext4 (исходный код и готовые файлы `.ko`): GPL-2.0-only, создан на основе [Mountify](https://github.com/backslashxx/mountify); см. [`module/lkm/README.md`](../module/lkm/README.md) и [`module/lkm/src/LICENSE`](../module/lkm/src/LICENSE).
- Подсистема VFS (модуль `hybridmount`): GPL-2.0-only, создана на основе [NoMount](https://github.com/maxsteeel/nomount); см. [`module/vfs/README.md`](../module/vfs/README.md), [`module/vfs/src/LICENSE`](../module/vfs/src/LICENSE) и [THIRD_PARTY.md](../THIRD_PARTY.md).
