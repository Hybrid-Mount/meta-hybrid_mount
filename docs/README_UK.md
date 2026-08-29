# Hybrid Mount

Hybrid Mount — метамодуль гібридного монтування для KernelSU та APatch. Під час завантаження він сканує інші модулі й для кожного елемента вибирає OverlayFS, Magic Mount або ігнорування відповідно до глобальних правил, правил модуля та шляху. Каталоги вихідних файлів модулів завжди використовуються лише для читання.

## Можливості

- OverlayFS і Magic Mount можна поєднувати на рівні окремих модулів і шляхів.
- Правила шляху мають пріоритет над типовими налаштуваннями модуля, а налаштування модуля — над глобальним типовим значенням.
- OverlayFS підтримує режими зберігання tmpfs і ext4.
- Для підготовки ext4 KernelSU використовує офіційний ioctl, що приховує вузли sysfs; APatch та інші середовища без KSU типово використовують вбудований сумісний LKM.
- Magic Mount підтримує файли, каталоги, символічні посилання, `.replace` і семантику whiteout.
- WebUI надає інтерфейси MD3 (типовий) і Miuix.
- Підтримуються arm64, armv7 та x86_64; інсталятор автоматично вибирає відповідний бінарний файл.

## Встановлення

Завантажте ZIP зі сторінки [Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases) і встановіть його через менеджер KernelSU або APatch. Під час першого встановлення виберіть типовий backend кнопками гучності. Під час оновлення файл `/data/adb/hybrid-mount/config.toml` зберігається.

## Налаштування

Типова конфігурація:

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

Шляхи в правилах указуються відносно кореня модуля. У правилах рівня модуля та шляху також можна використовувати `ignore`; глобальний типовий backend приймає лише `overlay` або `magic`. Один і той самий шлях до файлу не можна призначити обом backend монтування. Звичайні каталоги можуть використовуватися обома backend як спільні структурні вузли, але конфлікт файлу, типу або `.replace` негайно завершує етап планування під час запуску з помилкою. Зміни конфігурації набувають чинності після перезавантаження.

Ця маршрутизація не змінює наявну перевірку можливості `CONFIG_TMPFS_XATTR`. Під час встановлення в KernelSU весь каталог `lkm/` модуля видаляється, а під час роботи використовується лише офіційний ioctl `NukeExt4Sysfs`. Під час встановлення в APatch та інші середовища без KSU каталог LKM зберігається, і після монтування підготовки ext4 система типово намагається його використати. Вбудовані файли `.ko` підтримують лише aarch64. Для автоматичного вибору потрібен точний збіг гілки ядра та позначки Android/GKI; невідомі комбінації відхиляються. Сумісність ABI готових LKM усе одно потрібно перевіряти на відповідному реальному пристрої. Якщо пристрій аварійно завершить роботу під час `insmod`, постійний захисний маркер запобігатиме повторному завантаженню LKM під час наступного запуску, зберігаючи решту функцій Hybrid Mount. Матрицю підтримки, контрольні суми, джерела та ліцензії див. у [`module/lkm/README.md`](../module/lkm/README.md).

## Зворотний зв'язок

Перед встановленням або надсиланням повідомлення про проблему прочитайте [Повідомлення про використання](../USAGE_NOTICE.md). Додайте bugreport KernelSU/APatch, версію модуля та кроки для відтворення. Зв'язатися з нами можна через [GitHub Issues](https://github.com/Hybrid-Mount/meta-hybrid_mount/issues) або [групу Telegram](https://t.me/hybridmountchat).

## Мови / Languages

- [English](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_EN.md)
- [Español](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_ES.md)
- [Bahasa Indonesia](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_ID.md)
- [Italiano](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_IT.md)
- [日本語](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_JA.md)
- [Русский](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_RU.md)
- [Türkçe](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_TR.md)
- [Українська](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_UK.md)
- [Tiếng Việt](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_VI.md)
- [简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/README.md)
- [繁體中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_ZH_TW.md)

## Ліцензія

- Ядро (Rust і скрипти модуля): GPL-3.0-only (див. [`LICENSE`](../LICENSE)).
- WebUI: Apache-2.0 (див. [`webui/LICENSE`](../webui/LICENSE)).
- Необов'язковий LKM для sysfs ext4 (вихідний код і готові файли `.ko`): GPL-2.0-only, створений на основі [Mountify](https://github.com/backslashxx/mountify); див. [`module/lkm/README.md`](../module/lkm/README.md) і [`module/lkm/src/LICENSE`](../module/lkm/src/LICENSE).
