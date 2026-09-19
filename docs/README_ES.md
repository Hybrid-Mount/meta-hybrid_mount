# Hybrid Mount

<img src="../icon.svg" alt="Hybrid Mount logo" align="right" width="120" />

Hybrid Mount es un metamódulo de montaje híbrido para KernelSU y APatch. Durante el arranque, examina los demás módulos y selecciona OverlayFS, Magic Mount, VFS o ignorar para cada elemento según las reglas globales, de módulo y de ruta. Los directorios de origen de los módulos siempre se tratan como entradas de solo lectura.

## Funciones

- OverlayFS, Magic Mount y VFS pueden combinarse por módulo y por ruta.
- Las reglas de ruta tienen prioridad sobre los valores predeterminados del módulo, y estos tienen prioridad sobre el valor predeterminado global.
- OverlayFS admite los modos de almacenamiento tmpfs y ext4.
- Para la preparación ext4, KernelSU usa el ioctl oficial para ocultar los nodos sysfs; APatch y otros entornos que no son KSU usan de forma predeterminada el LKM de compatibilidad incluido.
- Magic Mount admite archivos, directorios, enlaces simbólicos, `.replace` y la semántica whiteout.
- VFS envía reglas de inyección al subsistema VFS propio de Hybrid Mount (módulo `hybridmount`) mediante el keyring. Es una implementación independiente y no interopera con el kernel de NoMount ni con su CLI nm. Las versiones incluyen el código fuente y un módulo arm64 precompilado por cada objetivo Android/GKI compatible, que el arranque carga cuando el kernel no lo incorpora; si falla, se degrada según `vfs_strict`. VFS no es un montaje real.
- La WebUI ofrece las interfaces MD3 (predeterminada) y Miuix.
- Se admiten arm64, armv7 y x86_64; el instalador selecciona automáticamente el binario correspondiente.

## Instalación

Descarga el ZIP desde [Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases) e instálalo con el administrador de KernelSU o APatch. En la primera instalación, usa las teclas de volumen para seleccionar el backend predeterminado. Las actualizaciones conservan `/data/adb/hybrid-mount/config.toml`.

## Configuración

Configuración predeterminada:

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

Las rutas de las reglas son relativas a la raíz del módulo. Las reglas de módulo y de ruta también pueden usar `ignore`; el backend global predeterminado acepta `overlay`, `magic` o `vfs`. VFS es una ruta de inyección, no un montaje real. Los conflictos de archivo, tipo o `.replace` hacen que la etapa de planificación del arranque falle inmediatamente. Los cambios de configuración se aplican después de reiniciar.

Este enrutamiento no modifica la comprobación de capacidad `CONFIG_TMPFS_XATTR` existente. En KernelSU, la instalación elimina por completo el directorio `lkm/` del módulo y, durante la ejecución, solo usa el ioctl oficial `NukeExt4Sysfs`. Las instalaciones de APatch y otros sistemas que no son KSU conservan el LKM y lo prueban de forma predeterminada después de montar la preparación ext4. Los archivos `.ko` incluidos solo admiten aarch64. La selección automática exige una coincidencia exacta de la línea del kernel y la etiqueta Android/GKI; las combinaciones desconocidas se rechazan. Los LKM precompilados deben validarse para comprobar la compatibilidad ABI en el dispositivo real correspondiente. Si el dispositivo falla durante `insmod`, un marcador persistente de protección evita que el LKM vuelva a cargarse en el siguiente arranque sin desactivar el resto de Hybrid Mount. Consulta [`module/lkm/README.md`](../module/lkm/README.md) para ver la matriz de compatibilidad, las sumas de comprobación, las fuentes y las licencias.

## Backend VFS

VFS es la ruta de inyección del propio Hybrid Mount en el lado del kernel, controlada a través del keyring por el módulo `hybridmount`. Es una implementación independiente y no interopera con NoMount.

**Cómo se identifica el proveedor.** La decisión de arranque se basa únicamente en una sonda de solo lectura del tipo de clave del kernel `hybridmount`: si responde con una versión compatible, el proveedor es utilizable. Por separado, `vfs-doctor` clasifica cómo está presente: una entrada en `/proc/modules` significa que lo registró un módulo cargable; un directorio `/sys/module/hybridmount` sin esa entrada significa que está compilado en la imagen del kernel; si no existe ninguna de las dos cosas, no hay ningún proveedor. La sonda es de solo lectura, por lo que `status` y `vfs-doctor` nunca desencadenan un `insmod`.

**Lógica de arranque.** Si el tipo de clave responde con una versión compatible, el proveedor se vincula y no se carga nada. Si ninguna regla selecciona VFS, tampoco se carga el módulo incluido. Si alguna regla sí selecciona VFS mientras la sonda permanece muda, la secuencia de arranque elige el módulo incluido que coincide exactamente con la línea del kernel y la etiqueta Android/GKI, lo carga y vuelve a sondear; si sigue sin estar disponible, todas las reglas `vfs` se degradan a `ignore`, o el arranque falla cuando `vfs_strict = true`. La carga se ejecuta antes de construir el plan de montaje, porque la planificación reescribe las reglas `vfs` a `ignore` mientras el proveedor está mudo y el ejecutor saldría antes de tiempo. Antes de `insmod` se escribe un marcador de protección y se elimina cuando el intento retorna, de modo que solo un fallo del kernel lo deja atrás; el siguiente arranque entonces rechaza un reintento automático hasta que el marcador se elimine manualmente.

**Integrar VFS en un kernel.** Las versiones incluyen un módulo aarch64 precompilado para cada objetivo Android/GKI compatible y lo cargan automáticamente, por lo que esos kernels no necesitan ningún paso de integración. Compílalo dentro del kernel cuando quieras evitar el `insmod`, o cuando tu línea del kernel no tenga precompilado. Desde la raíz de un árbol del kernel:

```sh
sh /path/to/metamodule/module/vfs/setup.sh
```

Esto copia las fuentes en `fs/hybridmount/` y las añade a `fs/Makefile` y `fs/Kconfig`; habilita `CONFIG_HYBRIDMOUNT=y` para compilarlo dentro del kernel o `=m` para compilarlo como módulo. `--cleanup` revierte todos los cambios. Un árbol que ya integra NoMount se rechaza: ambas implementaciones secuestran las operaciones de inodo y el kernel no impedirá que coexistan, ya que registran tipos de clave diferentes.

**Diagnóstico.** `/data/adb/modules/hybrid_mount/hybrid-mount vfs-doctor` informa del estado de presencia, de la versión que respondió el tipo de clave, de las versiones compatibles y, cuando un proveedor no es utilizable, del motivo.

## Comentarios y errores

Antes de instalar o informar de un problema, lee el [Aviso de uso](../USAGE_NOTICE.md). Incluye el bugreport de KernelSU/APatch, la versión del módulo y los pasos para reproducir el problema. Ponte en contacto mediante [GitHub Issues](https://github.com/Hybrid-Mount/meta-hybrid_mount/issues) o el [grupo de Telegram](https://t.me/hybridmountchat).

## Idiomas / Languages

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

## Agradecimientos

- Gracias a [Anatdx](https://github.com/Anatdx)
- Gracias a [Tools-cx-app](https://github.com/Tools-cx-app)
- Gracias a [KernelSU](https://github.com/tiann/KernelSU)
- Gracias a [MKSU de 5ec1cff](https://github.com/5ec1cff/KernelSU)
- Gracias a [ReSukiSU](https://github.com/ReSukiSU/ReSukiSU)
- Gracias a [meta-magic_mount-rs](https://github.com/Tools-cx-app/meta-magic_mount-rs)
- Gracias a [NoMount](https://github.com/maxsteeel/nomount)

## Licencia

- Núcleo (Rust y scripts del módulo): GPL-3.0-only (consulta [`LICENSE`](../LICENSE)).
- WebUI: Apache-2.0 (consulta [`webui/LICENSE`](../webui/LICENSE)).
- LKM sysfs ext4 opcional (código fuente y archivos `.ko` precompilados): GPL-2.0-only, derivado de [Mountify](https://github.com/backslashxx/mountify); consulta [`module/lkm/README.md`](../module/lkm/README.md) y [`module/lkm/src/LICENSE`](../module/lkm/src/LICENSE).
- Subsistema VFS (módulo `hybridmount`): GPL-2.0-only, derivado de [NoMount](https://github.com/maxsteeel/nomount); consulta [`module/vfs/README.md`](../module/vfs/README.md), [`module/vfs/src/LICENSE`](../module/vfs/src/LICENSE) y [THIRD_PARTY.md](../THIRD_PARTY.md).
