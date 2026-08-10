# Hybrid Mount

<img src="https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/main/icon.svg" align="right" width="120" />

![Language](https://img.shields.io/badge/Language-Rust-orange?style=flat-square&logo=rust)
![Platform](https://img.shields.io/badge/Platform-Android-green?style=flat-square&logo=android)
![License](https://img.shields.io/badge/License-GPL--3.0-blue?style=flat-square)
![Version](https://img.shields.io/github/v/tag/Hybrid-Mount/meta-hybrid_mount?label=Version&color=8A2BE2&style=flat-square)

Hybrid Mount è un metamodulo di orchestrazione dei mount per **KernelSU** e **APatch**.
Integra i file dei moduli nelle partizioni Android tramite un motore di policy unificato con due backend di mount:

- **OverlayFS**: mount a livelli con storage upper/work.
- **Magic Mount**: bind mount per sostituzione diretta dei percorsi.

Include una **WebUI SolidJS** per gestione grafica, monitoraggio in tempo reale e modifica della configurazione.

I pacchetti sono pubblicati in due varianti. Salvo indicazioni diverse, questo README descrive la variante Lite (quella predefinita).

**[English](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/README.md)** &nbsp; **[简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH.md)** &nbsp; **[繁體中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH_TW.md)** &nbsp; **[日本語](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_JP.md)** &nbsp; **[Español](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ES.md)** &nbsp; **[Italiano](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_IT.md)** &nbsp; **[Русский](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_RU.md)** &nbsp; **[Українська](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_UK.md)** &nbsp; **[Tiếng Việt](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_VI.md)**

---

## Indice

- [Funzionalità](#funzionalità)
- [Varianti di build](#varianti-di-build)
- [Avvio rapido](#avvio-rapido)
- [Modalità di mount](#modalità-di-mount)
- [WebUI](#webui)
- [Supporto lingue](#supporto-lingue)
- [Configurazione](#configurazione)
- [Riferimento policy](#riferimento-policy)
- [CLI](#cli)
- [Architettura](#architettura)
- [Build](#build)
- [Licenza](#licenza)

---

## Varianti di build

Hybrid Mount viene pubblicato in due varianti, ciascuna pensata per un caso d'uso diverso:

| Variante | Binario | WebUI | Daemon / CLI | Caso d'uso |
|----------|---------|-------|--------------|------------|
| **Lite (predefinita)** | Sì | Sì | Sì | Release predefinita: WebUI, daemon, CLI e backend OverlayFS e Magic Mount. |
| **Nano** | Sì | No | No | Utenti che vogliono solo orchestrazione tramite file di configurazione, senza daemon runtime, WebUI o CLI. |

### Lite

Lite è la variante predefinita. Include la WebUI SolidJS, il daemon su socket Unix (HTTP/SSE), la CLI e i backend OverlayFS e Magic Mount:

- Vuoi la WebUI e il motore di policy completo.
- Vuoi un pacchetto più piccolo mantenendo WebUI e interfaccia di gestione del daemon.

Le build Lite usano solo `control-plane` (`--no-default-features --features control-plane`).

### Nano

La variante `nano` (`--no-default-features`, nessun Cargo features) è guidata solo dal file di configurazione. Rimuove WebUI, daemon, CLI e infrastruttura di controllo; mantiene un binario ridotto che legge `config.toml`, genera un piano di mount, lo esegue e termina.

Nano usa `magic` come modalità predefinita. Durante l'installazione, la scelta tramite tasti volume scrive marker vuoti `overlay` o `magic` nella radice di ciascun modulo gestito. I nomi dei marker devono usare esattamente queste forme minuscole.

### Matrice funzionale

| Funzione | Lite | Nano |
| ---------- | ------ | ------ |
| Backend OverlayFS | Sì | Basato su marker |
| Backend Magic Mount | Sì | Sì, predefinito |
| WebUI | Sì | No |
| CLI | Sì | No |
| Daemon | Sì | No |
| Applicazione runtime della configurazione | Sì | No |
| Cargo features | solo `control-plane` | nessuno |
| Dimensione ZIP (approx.) | ~2 MB | ~1 MB |

## Funzionalità

- **Pianificazione deterministica**: i conflitti sono rilevati in fase di piano.
- **WebUI integrata**: gestione moduli, modifica configurazione e monitoraggio runtime.
- **Aggiornamenti runtime della configurazione**: le patch validate possono essere salvate e applicate subito.
- **Errori espliciti**: stati e configurazioni non validi falliscono immediatamente; `api config-reset` è un'azione esplicita.
- **Automazione**: protocollo JSON su Unix socket e API HTTP.

---

## Avvio rapido

1. Installa [KernelSU](https://kernelsu.org/) o [APatch](https://apatch.dev/) sul dispositivo.
2. Scarica lo ZIP `lite` o `nano` da [GitHub Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases).
3. Installa lo ZIP tramite il gestore moduli del root manager.
4. Alla prima installazione, scegli la modalità predefinita: Volume su seleziona OverlayFS, Volume giù seleziona Magic Mount e dopo 10 secondi senza input viene scelto OverlayFS. È l'unica interazione dell'installer; Nano salta questo passaggio.
5. Riavvia. Hybrid Mount rileverà l'ambiente e applicherà la policy selezionata.

```bash
# Controlla lo stato runtime
hybrid-mount daemon status

# Elenca i moduli rilevati
hybrid-mount api modules-list
```

Nella variante Lite, apri la WebUI dalla voce del modulo in KernelSU o APatch.

### Cambiare modalità di mount per un modulo

```toml
# /data/adb/hybrid-mount/config.toml
[rules.my_module]
default_mode = "magic"

[rules.my_module.paths]
"system/bin/problematic_binary" = "ignore"
```

---

## Modalità di mount

| Modalità | Backend | Uso consigliato |
|----------|---------|-----------------|
| `overlay` | OverlayFS | Moduli che aggiungono o sostituiscono file senza conflitti. Modalità predefinita. |
| `magic` | Bind mount | Sostituzione diretta per file. |
| `ignore` | Nessuno | Esclude percorsi specifici dal processo di mount. |

OverlayFS supporta `ext4` come storage persistente predefinito e `tmpfs` come alternativa volatile e leggera.
---

## WebUI

La WebUI basata su SolidJS è servita dal daemon tramite socket TCP locale con HTTP/SSE. CLI e client di automazione comunicano tramite Unix socket.

Funzioni principali:

- Dashboard di stato con statistiche, partizioni, modalità storage e stato del daemon.
- Gestione moduli e modifica interattiva delle policy.
- Editor `config.toml` con validazione e regole per percorso.

### Supporto lingue

La WebUI include questi locale:

- English (`en-US`, predefinito)
- Español (`es-ES`)
- Italiano (`it-IT`)
- 日本語 (`ja-JP`)
- Русский (`ru-RU`)
- Українська (`uk-UA`)
- Tiếng Việt (`vi-VN`)
- 简体中文 (`zh-CN`)
- 繁體中文 (`zh-TW`)

La documentazione README è disponibile in [English](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/README.md), [简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH.md), [繁體中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ZH_TW.md), [日本語](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_JP.md), [Español](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_ES.md), [Italiano](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_IT.md), [Русский](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_RU.md), [Українська](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_UK.md) e [Tiếng Việt](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/docs/README_VI.md).

---

## Configurazione

Percorso predefinito: `/data/adb/hybrid-mount/config.toml`.

| Campo | Tipo | Predefinito | Descrizione |
| --- | --- | --- | --- |
| `moduledir` | string | `/data/adb/modules` | Directory sorgente dei moduli. |
| `mountsource` | string | auto-detect | Ambiente runtime (`KSU`, `APatch`). |
| `overlay_mode` | `ext4` \| `tmpfs` | `ext4` | Storage upper/work di OverlayFS. |
| `disable_umount` | bool | `false` | Salta le operazioni umount, solo per debug. |
| `rules` | map | `{}` | Policy per modulo e per percorso. |

---

## Riferimento policy

Ordine di precedenza:

1. Override per percorso: `rules.<module>.paths["<path>"]`
2. Default del modulo: `rules.<module>.default_mode`
3. Default globale: `default_mode`

I marker riconosciuti includono `disable`, `remove`, `skip_mount`, `overlay`, `magic` e `.replace`. I nomi distinguono maiuscole e minuscole e devono corrispondere esattamente.

---

## CLI

```bash
hybrid-mount [OPTIONS] [COMMAND]
```

Sottocomandi comuni:

- `gen-config`: genera una configurazione predefinita.
- `logs`: stampa i log recenti del daemon.
- `api config-get` / `api config-set` / `api config-patch` / `api config-reset`: gestisce la configurazione.
- `api modules-list` / `api modules-apply`: legge e applica policy dei moduli.
- `daemon launch` / `daemon serve` / `daemon status` / `daemon stop`: gestisce il daemon.

---

## Architettura

Directory principali:

- `src/conf`: schema configurazione, loader TOML, CLI e handler.
- `src/domain`: tipi principali, regole e matching percorsi.
- `src/core`: inventario, pianificazione, daemon, API, startup e stato runtime.
- `webui`: WebUI SolidJS e i18n in 9 lingue.
- `xtask`: automazione build e release.

---

## Build

Requisiti:

- Rust nightly da `rust-toolchain.toml`
- Android NDK r27+ e `cargo-ndk`
- Node.js 20+ e pnpm per la WebUI

```bash
cargo run -p xtask -- build --release --flavor lite
cargo run -p xtask -- build --release --flavor nano
cargo run -p xtask -- build --release --skip-webui
./scripts/build-local.sh
cargo run -p xtask -- lint
cargo +nightly test
```

### CI gate e linting dei feature flag

---

## Licenza

Concesso in licenza con [GPL-3.0](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/LICENSE).
