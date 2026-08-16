# Hybrid Mount

<img src="https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/main/icon.svg" align="right" width="120" />

![Language](https://img.shields.io/badge/Language-Rust-orange?style=flat-square&logo=rust)
![Platform](https://img.shields.io/badge/Platform-Android-green?style=flat-square&logo=android)
![License](https://img.shields.io/badge/License-GPL--3.0-blue?style=flat-square)
![Version](https://img.shields.io/github/v/tag/Hybrid-Mount/meta-hybrid_mount?label=Version&color=8A2BE2&style=flat-square)

Hybrid Mount es un metamódulo de orquestación de montajes para **KernelSU** y **APatch**.
Integra archivos de módulos en particiones Android mediante un motor de políticas unificado con dos backends de montaje:

- **OverlayFS**: montajes por capas para compatibilidad amplia.
- **Magic Mount**: bind mount para reemplazo directo de rutas o fallback.

Incluye una **WebUI en SolidJS** para administración gráfica, monitoreo en vivo y edición de configuración.

Los paquetes se publican en dos variantes. Salvo que se indique lo contrario, este README describe la variante `lite`.

**[English](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/README.md)** &nbsp; **[简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH.md)** &nbsp; **[繁體中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH_TW.md)** &nbsp; **[日本語](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_JP.md)** &nbsp; **[Español](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ES.md)** &nbsp; **[Italiano](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_IT.md)** &nbsp; **[Русский](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_RU.md)** &nbsp; **[Українська](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_UK.md)** &nbsp; **[Tiếng Việt](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_VI.md)**

---

## Índice

- [Características](#características)
- [Variantes de compilación](#variantes-de-compilación)
- [Inicio rápido](#inicio-rápido)
- [Modos de montaje](#modos-de-montaje)
- [WebUI](#webui)
- [Soporte de idiomas](#soporte-de-idiomas)
- [Configuración](#configuración)
- [Referencia de políticas](#referencia-de-políticas)
- [CLI](#cli)
- [Arquitectura](#arquitectura)
- [Compilación](#compilación)
- [Notas operativas](#notas-operativas)
- [Licencia](#licencia)

---

## Variantes de compilación

| Variante | Binario | WebUI | Daemon / CLI | Caso de uso |
|----------|---------|-------|--------------|-------------|
| **Lite** | Sí | Sí | Sí | Usuarios que quieren WebUI y motor de políticas completo sin funciones stealth respaldadas por LKM. |
| **Nano** | Sí | No | No | Usuarios que solo necesitan orquestación por archivo de configuración, sin daemon runtime, WebUI ni CLI. |

### Lite

La variante `lite` es la variante por defecto. Conserva WebUI, daemon, CLI, OverlayFS y Magic Mount. Es adecuada cuando el kernel no carga LKMs externos o cuando no se necesitan capacidades runtime de hide/spoof.

### Nano

La variante `nano` (`--no-default-features`, sin Cargo features) funciona solo mediante configuración. Elimina WebUI, daemon, CLI e infraestructura de control; conserva un binario reducido que lee `config.toml`, genera un plan de montaje, lo ejecuta y termina.

Nano usa `magic` como modo predeterminado. Durante la instalación, la selección con teclas de volumen escribe marcadores vacíos `overlay` o `magic` en la raíz de cada módulo gestionado. Los nombres de marcadores se comparan sin distinguir mayúsculas y minúsculas.

### Matriz de funciones

| Función | Lite | Nano |
|---------|------|------|
| Backend OverlayFS | Sí | Basado en marcadores |
| Backend Magic Mount | Sí | Sí, predeterminado |
| WebUI | Sí | No |
| CLI | Sí | No |
| Daemon | Sí | No |
| Caché de configuración y aplicación runtime | Sí | No |
| Cargo features | solo `control-plane` | ninguno |
| Tamaño ZIP (aprox.) | ~2 MB | ~1 MB |

## Características

- **Dos backends, un motor de políticas**: asignación por ruta a OverlayFS o Magic Mount.
- **Planificación determinista**: los conflictos se detectan durante la planificación.
- **WebUI integrada**: gestión de módulos, edición de configuración y monitoreo runtime.
- **Caché de configuración**: parches incrementales y aplicación inmediata.
- **Recuperación práctica**: limpieza automática de archivos runtime obsoletos y reinicio con `api config-reset`.
- **Automatización**: protocolo JSON sobre Unix socket y API HTTP.

---

## Inicio rápido

1. Instala [KernelSU](https://kernelsu.org/) o [APatch](https://apatch.dev/) en el dispositivo.
2. Descarga el ZIP `lite` o `nano` desde [GitHub Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases).
3. Flashea el ZIP desde el instalador de módulos del gestor root.
4. Reinicia. Hybrid Mount detectará el entorno y aplicará la política overlay predeterminada.

```bash
# Comprobar estado runtime
hybrid-mount daemon status

# Listar módulos detectados
hybrid-mount api modules-list
```

En la variante Lite, abre la WebUI desde la entrada del módulo en KernelSU o APatch.

### Cambiar el modo de montaje de un módulo

```toml
# /data/adb/hybrid-mount/config.toml
[rules.my_module]
default_mode = "magic"

[rules.my_module.paths]
"system/bin/problematic_binary" = "ignore"
```

---

## Modos de montaje

| Modo | Backend | Uso recomendado |
|------|---------|-----------------|
| `overlay` | OverlayFS | Módulos que agregan o reemplazan archivos sin conflictos. Modo predeterminado. |
| `magic` | Bind mount | Reemplazo directo por archivo. |
| `ignore` | Ninguno | Excluir rutas específicas del procesamiento de montaje. |

OverlayFS admite `ext4` como almacenamiento persistente predeterminado y `tmpfs` como alternativa volátil y ligera.
---

## WebUI

La WebUI basada en SolidJS se sirve desde el daemon mediante un socket TCP local con HTTP/SSE. La CLI y los clientes de automatización usan Unix socket.

Funciones principales:

- Panel de estado con estadísticas, particiones, modo de almacenamiento y salud del daemon.
- Gestión de módulos y cambio interactivo de políticas.
- Editor de `config.toml` con validación y reglas por ruta.

### Soporte de idiomas

La WebUI incluye estos locales:

- English (`en-US`, predeterminado)
- Español (`es-ES`)
- Italiano (`it-IT`)
- 日本語 (`ja-JP`)
- Русский (`ru-RU`)
- Українська (`uk-UA`)
- Tiếng Việt (`vi-VN`)
- 简体中文 (`zh-CN`)
- 繁體中文 (`zh-TW`)

La documentación README está disponible en [English](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/README.md), [简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH.md), [繁體中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH_TW.md), [日本語](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_JP.md), [Español](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ES.md), [Italiano](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_IT.md), [Русский](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_RU.md), [Українська](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_UK.md) y [Tiếng Việt](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_VI.md).

---

## Configuración

Ruta predeterminada: `/data/adb/hybrid-mount/config.toml`.

| Campo | Tipo | Predeterminado | Descripción |
| --- | --- | --- | --- |
| `moduledir` | string | `/data/adb/modules` | Directorio fuente de módulos. |
| `mountsource` | string | autodetección | Entorno runtime (`KSU`, `APatch`). |
| `overlay_mode` | `ext4` \| `tmpfs` | `ext4` | Almacenamiento upper/work de OverlayFS. |
| `disable_umount` | bool | `false` | Omite operaciones umount, solo para depuración. |
| `default_mode` | `overlay` \| `magic` | `overlay` | Política global predeterminada. |
| `daemon_startup_mode` | `on-demand` \| `persistent` | `on-demand` | Modo de inicio del daemon. |
| `rules` | map | `{}` | Políticas por módulo y por ruta. |

---


## Referencia de políticas

Orden de precedencia:

1. Anulación por ruta: `rules.<module>.paths["<path>"]`
2. Valor predeterminado del módulo: `rules.<module>.default_mode`
3. Valor predeterminado global: `default_mode`

Los marcadores de módulo reconocidos incluyen `disable`, `remove`, `skip_mount`, `mount_error`, `overlay`, `magic` y `.replace`. Los nombres de marcadores se comparan sin distinguir mayúsculas y minúsculas.

---

## CLI

```bash
hybrid-mount [OPTIONS] [COMMAND]
```

Subcomandos comunes:

- `gen-config`: generar configuración predeterminada.
- `logs`: imprimir logs recientes del daemon.
- `api config-get` / `api config-set` / `api config-patch` / `api config-reset`: gestionar configuración.
- `api modules-list` / `api modules-apply`: consultar y aplicar políticas de módulos.
- `daemon launch` / `daemon serve` / `daemon status` / `daemon stop`: gestionar el daemon.

---

## Arquitectura

Hybrid Mount lee `config.toml`, descubre el inventario de módulos, genera un plan de montaje según reglas de ruta, módulo y globales, y ejecuta el plan mediante OverlayFS o Magic Mount. El ejecutor se basa en una **máquina de estados tipada** (`src/core/controller.rs`): `MountController<Init> → StorageReady → Planned → Executed`. Cada transición representa una etapa del pipeline. La variante Lite persiste el estado runtime y lo expone a WebUI y CLI mediante el daemon.

Directorios principales:

- `src/conf`: esquema de configuración, carga TOML, CLI y handlers.
- `src/domain`: tipos principales, reglas y coincidencia de rutas.
- `src/core`: inventario, planificación, daemon, API, inicio y estado runtime.
- `src/mount`: backends OverlayFS y Magic Mount.
- `src/sys`: syscalls de montaje.
- `webui`: WebUI SolidJS e i18n de 9 idiomas.
- `xtask`: automatización de build y release.

---

## Compilación

Requisitos:

- Rust nightly desde `rust-toolchain.toml`
- Android NDK r27+ y `cargo-ndk`
- Node.js 20+ y pnpm para la WebUI

```bash
cargo run -p xtask -- build --release --flavor lite
cargo run -p xtask -- build --release --flavor nano
cargo run -p xtask -- build --release --skip-webui
./scripts/build-local.sh
cargo run -p xtask -- lint
cargo +nightly test
```

### Puertas CI y linting de feature flags

Cada cambio debe pasar: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets --workspace`, WebUI `pnpm lint` + `pnpm test`, y verificación de encabezado de licencia. Verifica también que **lite** (`--no-default-features --features control-plane`) y **nano** (`--no-default-features`) compilen. El código del daemon/CLI/WebUI debe estar tras `#[cfg(feature = "control-plane")]`.

---

## Notas operativas

- Las instalaciones nuevas detectan `mountsource` automáticamente.
- Ante una configuración dañada, usa `hybrid-mount api config-reset` y reaplica reglas gradualmente.
- `api config-patch --apply-runtime` permite aplicar cambios inmediatamente.

---

## Licencia

Licenciado bajo [GPL-3.0](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/LICENSE).
