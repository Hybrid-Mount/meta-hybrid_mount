# Hybrid Mount

Hybrid Mount es un metamódulo de montaje híbrido para KernelSU y APatch. Durante el arranque, examina los demás módulos y selecciona OverlayFS, Magic Mount o ignorar para cada elemento según las reglas globales, de módulo y de ruta. Los directorios de origen de los módulos siempre se tratan como entradas de solo lectura.

## Funciones

- OverlayFS y Magic Mount pueden combinarse por módulo y por ruta.
- Las reglas de ruta tienen prioridad sobre los valores predeterminados del módulo, y estos tienen prioridad sobre el valor predeterminado global.
- OverlayFS admite los modos de almacenamiento tmpfs y ext4.
- Para la preparación ext4, KernelSU usa el ioctl oficial para ocultar los nodos sysfs; APatch y otros entornos que no son KSU usan de forma predeterminada el LKM de compatibilidad incluido.
- Magic Mount admite archivos, directorios, enlaces simbólicos, `.replace` y la semántica whiteout.
- La WebUI ofrece las interfaces MD3 (predeterminada) y Miuix.
- Se admiten arm64, armv7 y x86_64; el instalador selecciona automáticamente el binario correspondiente.

## Instalación

Descarga el ZIP desde [Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases) e instálalo con el administrador de KernelSU o APatch. En la primera instalación, usa las teclas de volumen para seleccionar el backend predeterminado. Las actualizaciones conservan `/data/adb/hybrid-mount/config.toml`.

## Configuración

Configuración predeterminada:

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

Las rutas de las reglas son relativas a la raíz del módulo. Las reglas de módulo y de ruta también pueden usar `ignore`; el backend global predeterminado solo acepta `overlay` o `magic`. Una misma ruta de archivo no puede asignarse a ambos backends de montaje. Los directorios normales pueden compartirse como nodos estructurales entre ambos backends, mientras que los conflictos de archivo, tipo o `.replace` hacen que la etapa de planificación del arranque falle inmediatamente. Los cambios de configuración se aplican después de reiniciar.

Este enrutamiento no modifica la comprobación de capacidad `CONFIG_TMPFS_XATTR` existente. En KernelSU, la instalación elimina por completo el directorio `lkm/` del módulo y, durante la ejecución, solo usa el ioctl oficial `NukeExt4Sysfs`. Las instalaciones de APatch y otros sistemas que no son KSU conservan el LKM y lo prueban de forma predeterminada después de montar la preparación ext4. Los archivos `.ko` incluidos solo admiten aarch64. La selección automática exige una coincidencia exacta de la línea del kernel y la etiqueta Android/GKI; las combinaciones desconocidas se rechazan. Los LKM precompilados deben validarse para comprobar la compatibilidad ABI en el dispositivo real correspondiente. Si el dispositivo falla durante `insmod`, un marcador persistente de protección evita que el LKM vuelva a cargarse en el siguiente arranque sin desactivar el resto de Hybrid Mount. Consulta [`module/lkm/README.md`](../module/lkm/README.md) para ver la matriz de compatibilidad, las sumas de comprobación, las fuentes y las licencias.

## Comentarios y errores

Antes de instalar o informar de un problema, lee el [Aviso de uso](../USAGE_NOTICE.md). Incluye el bugreport de KernelSU/APatch, la versión del módulo y los pasos para reproducir el problema. Ponte en contacto mediante [GitHub Issues](https://github.com/Hybrid-Mount/meta-hybrid_mount/issues) o el [grupo de Telegram](https://t.me/hybridmountchat).

## Idiomas / Languages

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

## Licencia

- Núcleo (Rust y scripts del módulo): GPL-3.0-only (consulta [`LICENSE`](../LICENSE)).
- WebUI: Apache-2.0 (consulta [`webui/LICENSE`](../webui/LICENSE)).
- LKM sysfs ext4 opcional (código fuente y archivos `.ko` precompilados): GPL-2.0-only, derivado de [Mountify](https://github.com/backslashxx/mountify); consulta [`module/lkm/README.md`](../module/lkm/README.md) y [`module/lkm/src/LICENSE`](../module/lkm/src/LICENSE).
