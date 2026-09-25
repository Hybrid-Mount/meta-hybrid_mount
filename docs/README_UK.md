# Hybrid Mount

<img src="../icon.svg" alt="Hybrid Mount logo" align="right" width="120" />

Hybrid Mount — метамодуль гібридного монтування для KernelSU та APatch. Під час завантаження він сканує інші модулі й для кожного елемента вибирає OverlayFS, Magic Mount, VFS або ігнорування відповідно до глобальних правил, правил модуля та шляху. Каталоги вихідних файлів модулів завжди використовуються лише для читання.

## Можливості

- OverlayFS, Magic Mount і VFS можна поєднувати на рівні окремих модулів і шляхів.
- Правила шляху мають пріоритет над типовими налаштуваннями модуля, а налаштування модуля — над глобальним типовим значенням.
- OverlayFS підтримує режими зберігання tmpfs і ext4.
- Для підготовки ext4 KernelSU використовує офіційний ioctl, що приховує вузли sysfs; APatch та інші середовища без KSU типово використовують вбудований сумісний LKM.
- Magic Mount підтримує файли, каталоги, символічні посилання, `.replace` і семантику whiteout.
- VFS передає правила ін'єкції до власної підсистеми VFS Hybrid Mount (модуль `hybridmount`) через keyring. Це незалежна реалізація, не сумісна з ядром NoMount та його CLI nm. Релізи містять вихідний код і попередньо скомпільований модуль arm64 для кожної підтримуваної цілі Android/GKI; він завантажується під час завантаження, якщо ядро його не містить. У разі невдачі поведінка визначається `vfs_strict`. VFS не є справжнім монтуванням.
- WebUI надає інтерфейси MD3 (типовий) і Miuix.
- Підтримуються arm64, armv7, x86_64 та riscv64; інсталятор автоматично вибирає відповідний бінарний файл. Для збірки riscv64 потрібен Android NDK r27 або новіший.

## Встановлення

Завантажте ZIP зі сторінки [Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases) і встановіть його через менеджер KernelSU або APatch. Під час першого встановлення виберіть типовий backend кнопками гучності. Під час оновлення файл `/data/adb/hybrid-mount/config.toml` зберігається.

## Налаштування

Типова конфігурація:

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

Шляхи в правилах указуються відносно кореня модуля. У правилах рівня модуля та шляху також можна використовувати `ignore`; глобальний типовий backend приймає `overlay`, `magic` або `vfs`. VFS — це шлях ін’єкції, а не справжнє монтування. Конфлікт файлу, типу або `.replace` негайно завершує етап планування під час запуску з помилкою. Зміни конфігурації набувають чинності після перезавантаження.

Ця маршрутизація не змінює наявну перевірку можливості `CONFIG_TMPFS_XATTR`. Під час встановлення в KernelSU весь каталог `lkm/` модуля видаляється, а під час роботи використовується лише офіційний ioctl `NukeExt4Sysfs`. Під час встановлення в APatch та інші середовища без KSU каталог LKM зберігається, і після монтування підготовки ext4 система типово намагається його використати. Вбудовані файли `.ko` підтримують лише aarch64. Для автоматичного вибору потрібен точний збіг гілки ядра та позначки Android/GKI; невідомі комбінації відхиляються. Сумісність ABI готових LKM усе одно потрібно перевіряти на відповідному реальному пристрої. Якщо пристрій аварійно завершить роботу під час `insmod`, постійний захисний маркер запобігатиме повторному завантаженню LKM під час наступного запуску, зберігаючи решту функцій Hybrid Mount. Матрицю підтримки, контрольні суми, джерела та ліцензії див. у [`module/lkm/README.md`](../module/lkm/README.md).

## Бекенд VFS

VFS — це власний шлях ін'єкції Hybrid Mount на боці ядра, яким керує через keyring модуль `hybridmount`. Це незалежна реалізація, не сумісна з NoMount.

**Як визначається провайдер.** Рішення про завантаження спирається виключно на перевірку типу ключа ядра `hybridmount` лише для читання: якщо він відповідає підтримуваною версією, провайдер придатний. Окремо `vfs-doctor` визначає спосіб його присутності — запис у `/proc/modules` означає, що його зареєстрував завантажуваний модуль, каталог `/sys/module/hybridmount` без такого запису означає, що його вбудовано в образ ядра, а якщо немає ні того, ні іншого — провайдер відсутній. Перевірка виконується лише для читання, тому `status` і `vfs-doctor` ніколи не запускають `insmod`.

**Логіка завантаження.** Якщо тип ключа відповідає підтримуваною версією, провайдер прив'язується і нічого не завантажується. Якщо жодне правило не вибирає VFS, вбудований модуль також не завантажується. Якщо правило все ж вибирає VFS, а перевірка мовчить, конвеєр вибирає вбудований модуль, що точно відповідає гілці ядра та позначці Android/GKI, завантажує його й перевіряє знову; якщо він усе ще недоступний, кожне правило `vfs` знижується до `ignore` або завантаження завершується помилкою за `vfs_strict = true`. Завантаження відбувається до побудови плану монтування, оскільки, поки провайдер мовчить, планування переписує правила `vfs` на `ignore`, і виконавець тоді завершився б достроково. Захисний маркер записується перед `insmod` і знімається після повернення спроби, тому його залишає лише збій ядра; тоді наступне завантаження відмовляється від автоматичної повторної спроби, доки маркер не видалять вручну.

**Інтеграція VFS у ядро.** У релізах постачається попередньо скомпільований модуль aarch64 для кожної підтримуваної цілі Android/GKI, і він завантажується автоматично, тож таким ядрам не потрібен етап інтеграції. Вбудуйте його, якщо хочете уникнути `insmod` або якщо для вашої гілки ядра немає готового модуля. З кореня дерева ядра:

```sh
curl -LSs "https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/dev/module/vfs/setup.sh" | bash
```

`--cleanup`:

```sh
curl -LSs "https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/dev/module/vfs/setup.sh" | bash -s -- --cleanup
```

Скрипт копіює джерела до `fs/hybridmount/` і додає їх до `fs/Makefile` та `fs/Kconfig`; увімкніть `CONFIG_HYBRIDMOUNT=y`, щоб вбудувати його, або `=m`, щоб зібрати як модуль. `bash -s -- --cleanup` скасовує всі зміни. Дерево, у яке вже інтегровано NoMount, відхиляється: обидві реалізації перехоплюють операції з inode, і ядро не завадить їм співіснувати, оскільки вони реєструють різні типи ключів.

**Діагностика.** `/data/adb/modules/hybrid_mount/hybrid-mount vfs-doctor` повідомляє стан присутності, версію, яку повернув тип ключа, підтримувані версії та причину, з якої провайдер непридатний, коли це так.

## Зворотний зв'язок

Перед встановленням або надсиланням повідомлення про проблему прочитайте [Повідомлення про використання](../USAGE_NOTICE.md). Додайте bugreport KernelSU/APatch, версію модуля та кроки для відтворення. Зв'язатися з нами можна через [GitHub Issues](https://github.com/Hybrid-Mount/meta-hybrid_mount/issues) або [групу Telegram](https://t.me/hybridmountchat).

## Мови / Languages

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

## Подяки

- Дякуємо [Anatdx](https://github.com/Anatdx)
- Дякуємо [Tools-cx-app](https://github.com/Tools-cx-app)
- Дякуємо [KernelSU](https://github.com/tiann/KernelSU)
- Дякуємо [MKSU від 5ec1cff](https://github.com/5ec1cff/KernelSU)
- Дякуємо [ReSukiSU](https://github.com/ReSukiSU/ReSukiSU)
- Дякуємо [meta-magic_mount-rs](https://github.com/Tools-cx-app/meta-magic_mount-rs)
- Дякуємо [NoMount](https://github.com/maxsteeel/nomount)

## Ліцензія

- Ядро (Rust і скрипти модуля): GPL-3.0-only (див. [`LICENSE`](../LICENSE)).
- WebUI: Apache-2.0 (див. [`webui/LICENSE`](../webui/LICENSE)).
- Необов'язковий LKM для sysfs ext4 (вихідний код і готові файли `.ko`): GPL-2.0-only, створений на основі [Mountify](https://github.com/backslashxx/mountify); див. [`module/lkm/README.md`](../module/lkm/README.md) і [`module/lkm/src/LICENSE`](../module/lkm/src/LICENSE).
- Підсистема VFS (модуль `hybridmount`): GPL-2.0-only, створена на основі [NoMount](https://github.com/maxsteeel/nomount); див. [`module/vfs/README.md`](../module/vfs/README.md), [`module/vfs/src/LICENSE`](../module/vfs/src/LICENSE) та [THIRD_PARTY.md](../THIRD_PARTY.md).
