# Hybrid Mount

<img src="https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/main/icon.svg" align="right" width="120" />

![Language](https://img.shields.io/badge/Language-Rust-orange?style=flat-square&logo=rust)
![Platform](https://img.shields.io/badge/Platform-Android-green?style=flat-square&logo=android)
![License](https://img.shields.io/badge/License-Apache--2.0-blue?style=flat-square)
![Version](https://img.shields.io/github/v/tag/Hybrid-Mount/meta-hybrid_mount?label=Version&color=8A2BE2&style=flat-square)

Hybrid Mount è un metamodulo di orchestrazione dei mount per **KernelSU** e **APatch**.
Integra i file dei moduli nelle partizioni Android tramite un motore di policy unificato con tre backend di mount:

- **OverlayFS**: mount a livelli per ampia compatibilità.
- **Magic Mount**: bind mount per sostituzione diretta dei percorsi o fallback.
- **Kasumi**: routing basato su LKM con funzionalità runtime di hide, spoof e stealth.

Include una **WebUI SolidJS** per gestione grafica, monitoraggio in tempo reale e modifica della configurazione.

I pacchetti sono pubblicati in tre varianti. Salvo indicazioni diverse, questo README descrive la variante `full`.

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
- [Kasumi](#kasumi)
- [Riferimento policy](#riferimento-policy)
- [CLI](#cli)
- [Architettura](#architettura)
- [Build](#build)
- [Note operative](#note-operative)
- [Licenza](#licenza)

---

## Varianti di build

| Variante | Binario | WebUI | Daemon / CLI | Kasumi LKM | Caso d'uso |
|----------|---------|-------|--------------|------------|------------|
| **Full** | Sì | Sì | Sì | Sì | Utenti che richiedono routing Kasumi o funzionalità hide/spoof. |
| **Lite** | Sì | Sì | Sì | No | Utenti che vogliono WebUI e motore di policy completo senza funzioni stealth basate su LKM. |
| **Nano** | Sì | No | No | No | Utenti che vogliono solo orchestrazione tramite file di configurazione, senza daemon runtime, WebUI o CLI. |

### Full

La variante `full` include tutti i backend supportati (OverlayFS, Magic Mount, Kasumi), la WebUI SolidJS, il daemon con Unix socket e HTTP/SSE, la CLI e gli asset Kasumi LKM. Compilato con Cargo features `kasumi` (che implica `control-plane`).

### Lite

La variante `lite` (`--no-default-features --features control-plane`) rimuove Kasumi LKM e tutte le funzionalità correlate, ma mantiene WebUI, daemon, CLI, OverlayFS e Magic Mount. È indicata quando il kernel non supporta LKM esterni o quando non servono capacità runtime di hide/spoof.

### Nano

La variante `nano` (`--no-default-features`, nessun Cargo features) è guidata solo dal file di configurazione. Rimuove WebUI, daemon, CLI e infrastruttura di controllo; mantiene un binario ridotto che legge `config.toml`, genera un piano di mount, lo esegue e termina.

Nano usa `magic` come modalità predefinita. Durante l'installazione, la scelta tramite tasti volume scrive marker vuoti `overlay` o `magic` nella radice di ciascun modulo gestito. I nomi dei marker sono confrontati senza distinzione tra maiuscole e minuscole.

### Matrice funzionale

| Funzione | Full | Lite | Nano |
|----------|------|------|------|
| Backend OverlayFS | Sì | Sì | Basato su marker |
| Backend Magic Mount | Sì | Sì | Sì, predefinito |
| Backend Kasumi | Sì | No | No |
| WebUI | Sì | Sì | No |
| CLI | Sì | Sì | No |
| Daemon | Sì | Sì | No |
| Cache configurazione e apply runtime | Sì | Sì | No |
| Kasumi hide/spoof/stealth | Sì | No | No |
| Autoload LKM | Sì | No | No |
| Cargo features | `kasumi` (implica `control-plane`) | solo `control-plane` | nessuno |
| Dimensione ZIP (approx.) | ~4 MB | ~2 MB | ~1 MB |

## Funzionalità

- **Tre backend, un motore di policy**: assegnazione per percorso a OverlayFS, Magic Mount o Kasumi.
- **Pianificazione deterministica**: i conflitti sono rilevati in fase di piano.
- **WebUI integrata**: gestione moduli, modifica configurazione e monitoraggio runtime.
- **Integrazione Kasumi runtime**: autoload LKM, routing mirror, mount hide, spoof maps/statfs, UID hiding, uname spoof e regole kstat.
- **Cache configurazione**: patch incrementali e applicazione immediata.
- **Recupero pratico**: pulizia automatica dei file runtime obsoleti e reset con `api config-reset`.
- **Automazione**: protocollo JSON su Unix socket e API HTTP.

---

## Avvio rapido

1. Installa [KernelSU](https://kernelsu.org/) o [APatch](https://apatch.dev/) sul dispositivo.
2. Scarica lo ZIP `full`, `lite` o `nano` da [GitHub Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases).
3. Installa lo ZIP tramite il gestore moduli del root manager.
4. Riavvia. Hybrid Mount rileverà l'ambiente e applicherà la policy overlay predefinita.

```bash
# Controlla lo stato runtime
hybrid-mount daemon status

# Elenca i moduli rilevati
hybrid-mount api modules-list
```

Nelle varianti Full/Lite, apri la WebUI dalla voce del modulo in KernelSU o APatch.

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
| `kasumi` | Kasumi LKM | Routing mirror esplicito o funzionalità runtime hide/spoof. |
| `ignore` | Nessuno | Esclude percorsi specifici dal processo di mount. |

OverlayFS supporta `ext4` come storage persistente predefinito e `tmpfs` come alternativa volatile e leggera.
---

## WebUI

La WebUI basata su SolidJS è servita dal daemon tramite socket TCP locale con HTTP/SSE. CLI e client di automazione comunicano tramite Unix socket.

Funzioni principali:

- Dashboard di stato con statistiche, partizioni, modalità storage e stato del daemon.
- Gestione moduli e modifica interattiva delle policy.
- Editor `config.toml` con validazione e regole per percorso.
- Pannello Kasumi per stato LKM, regole e opzioni spoof in Full.

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
| `default_mode` | `overlay` \| `magic` \| `kasumi` | `overlay` | Policy globale predefinita. |
| `daemon_startup_mode` | `on-demand` \| `persistent` | `on-demand` | Modalità di avvio del daemon. |
| `rules` | map | `{}` | Policy per modulo e per percorso. |

---

## Kasumi

Kasumi è il backend basato su LKM. Oltre al routing dei mount, fornisce funzioni hide e spoof. Viene usato quando `kasumi.enabled = true` e il piano contiene regole Kasumi, oppure quando sono configurate funzioni ausiliarie come hidexattr, mount hide, maps/statfs spoof, UID hiding, uname spoof, sostituzione cmdline o regole kstat/user hide.

```bash
hybrid-mount kasumi status
hybrid-mount kasumi features
hybrid-mount kasumi list
hybrid-mount lkm status
hybrid-mount kasumi apply-config-runtime
hybrid-mount kasumi clear
```

---

## Riferimento policy

Ordine di precedenza:

1. Override per percorso: `rules.<module>.paths["<path>"]`
2. Default del modulo: `rules.<module>.default_mode`
3. Default globale: `default_mode`

I marker riconosciuti includono `disable`, `remove`, `skip_mount`, `mount_error`, `overlay`, `magic` e `.replace`. I nomi sono confrontati senza distinzione tra maiuscole e minuscole.

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
- `kasumi ...`: gestisce Kasumi.
- `lkm load` / `lkm unload` / `lkm status`: gestisce LKM.

---

## Architettura

Hybrid Mount legge `config.toml`, scopre l'inventario dei moduli, genera un piano di mount in base a regole di percorso, modulo e globali, quindi lo esegue tramite OverlayFS, Magic Mount o Kasumi. L'esecutore è guidato da una **macchina a stati tipata** (`src/core/controller.rs`): `MountController<Init> → StorageReady → Planned → Executed`. Ogni transizione rappresenta una fase del pipeline. Le varianti Full/Lite persistono lo stato runtime e lo espongono a WebUI e CLI tramite daemon.

Directory principali:

- `src/conf`: schema configurazione, loader TOML, CLI e handler.
- `src/domain`: tipi principali, regole e matching percorsi.
- `src/core`: inventario, pianificazione, daemon, API, startup e stato runtime.
- `src/mount`: backend OverlayFS, Magic Mount e Kasumi.
- `src/sys`: syscall mount, LKM e Kasumi UAPI.
- `webui`: WebUI SolidJS e i18n in 9 lingue.
- `xtask`: automazione build e release.

---

## Build

Requisiti:

- Rust nightly da `rust-toolchain.toml`
- Android NDK r27+ e `cargo-ndk`
- Node.js 20+ e pnpm per la WebUI

```bash
cargo run -p xtask -- build --release --flavor full
cargo run -p xtask -- build --release --flavor lite
cargo run -p xtask -- build --release --flavor nano
cargo run -p xtask -- build --release --skip-webui
./scripts/build-local.sh
cargo run -p xtask -- lint
cargo +nightly test
```

### CI gate e linting dei feature flag

Ogni modifica deve superare: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets --workspace`, WebUI `pnpm lint` + `pnpm test`, e controllo dell'intestazione di licenza. `cargo clippy --all-features` verifica solo la variante `full`; assicurati anche che le combinazioni **lite** (`--no-default-features --features control-plane`) e **nano** (`--no-default-features`) compilino. Il codice Kasumi va protetto con `#[cfg(feature = "kasumi")]`; il codice daemon/CLI/WebUI con `#[cfg(feature = "control-plane")]`.

---

## Note operative

- Le nuove installazioni rilevano `mountsource` automaticamente.
- In caso di configurazione errata, usa `hybrid-mount api config-reset` e riapplica le regole gradualmente.
- `api config-patch --apply-runtime` applica subito modifiche parziali.
- In Full, Kasumi LKM deve corrispondere al kernel in esecuzione; usa `lkm_kmi_override` se il KMI rilevato non è corretto.
- `kasumi clear` pulisce lo stato runtime e rilascia la connessione kernel; alcune regole lato kernel possono persistere fino al reload del LKM.

---

## Licenza

Concesso in licenza con [Apache-2.0](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/main/LICENSE).
