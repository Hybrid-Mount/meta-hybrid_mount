# Hybrid Mount

<img src="../icon.svg" alt="Hybrid Mount logo" align="right" width="120" />

Hybrid Mount è un metamodulo di montaggio ibrido per KernelSU e APatch. Durante l'avvio analizza gli altri moduli e seleziona OverlayFS, Magic Mount, VFS oppure ignora ogni elemento in base alle regole globali, del modulo e del percorso. Le directory sorgente dei moduli vengono sempre trattate come input di sola lettura.

## Funzionalità

- OverlayFS, Magic Mount e VFS possono essere combinati per modulo e per percorso.
- Le regole del percorso hanno la precedenza sui valori predefiniti del modulo, che a loro volta hanno la precedenza sul valore predefinito globale.
- OverlayFS supporta le modalità di archiviazione tmpfs ed ext4.
- Per lo staging ext4, KernelSU usa l'ioctl ufficiale per nascondere i nodi sysfs; APatch e gli altri ambienti non KSU usano per impostazione predefinita l'LKM di compatibilità incluso.
- Magic Mount supporta file, directory, collegamenti simbolici, `.replace` e la semantica whiteout.
- VFS invia le regole di iniezione al sottosistema VFS proprietario di Hybrid Mount (modulo `hybridmount`) tramite il keyring. È un'implementazione indipendente e non interopera con il kernel di NoMount né con la sua CLI nm. Le release includono i sorgenti e un modulo arm64 precompilato per ogni target Android/GKI supportato, caricato all'avvio quando il kernel non lo integra; in caso di errore si degrada secondo `vfs_strict`. VFS non è un mount reale.
- La WebUI offre le interfacce MD3 (predefinita) e Miuix.
- Sono supportate le architetture arm64, armv7 e x86_64; il programma di installazione seleziona automaticamente il binario corretto.

## Installazione

Scarica lo ZIP da [Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases) e installalo con il gestore KernelSU o APatch. Durante la prima installazione, usa i tasti del volume per selezionare il backend predefinito. Gli aggiornamenti mantengono `/data/adb/hybrid-mount/config.toml`.

## Configurazione

Configurazione predefinita:

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

I percorsi delle regole sono relativi alla radice del modulo. Le regole a livello di modulo e di percorso possono usare anche `ignore`; il backend globale predefinito accetta `overlay`, `magic` o `vfs`. VFS è un percorso di iniezione, non un montaggio reale. I conflitti di file, tipo o `.replace` causano l'arresto immediato della pianificazione all'avvio. Le modifiche alla configurazione diventano effettive dopo il riavvio.

Questo instradamento non modifica il controllo esistente della funzionalità `CONFIG_TMPFS_XATTR`. Su KernelSU, l'installazione elimina l'intera directory `lkm/` del modulo e durante l'esecuzione usa solo l'ioctl ufficiale `NukeExt4Sysfs`. Le installazioni APatch e non KSU mantengono l'LKM e tentano di usarlo per impostazione predefinita dopo il montaggio dello staging ext4. I file `.ko` inclusi supportano solo aarch64. La selezione automatica richiede una corrispondenza esatta della linea del kernel e del tag Android/GKI; le combinazioni sconosciute vengono rifiutate. Gli LKM precompilati devono comunque essere verificati sul dispositivo reale corrispondente per la compatibilità ABI. Se il dispositivo si arresta in modo anomalo durante `insmod`, un indicatore persistente di protezione impedisce un nuovo caricamento dell'LKM all'avvio successivo, mantenendo operative le altre funzioni di Hybrid Mount. Consulta [`module/lkm/README.md`](../module/lkm/README.md) per la matrice di supporto, i checksum, le fonti e le licenze.

## Segnalazioni

Prima dell'installazione o di segnalare un problema, leggi l'[Avviso d'uso](../USAGE_NOTICE.md). Includi il bugreport di KernelSU/APatch, la versione del modulo e i passaggi per riprodurre il problema. Contattaci tramite [GitHub Issues](https://github.com/Hybrid-Mount/meta-hybrid_mount/issues) o il [gruppo Telegram](https://t.me/hybridmountchat).

## Lingue / Languages

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

## Licenza

- Core (Rust e script del modulo): GPL-3.0-only (vedi [`LICENSE`](../LICENSE)).
- WebUI: Apache-2.0 (vedi [`webui/LICENSE`](../webui/LICENSE)).
- LKM sysfs ext4 opzionale (sorgenti e file `.ko` precompilati): GPL-2.0-only, derivato da [Mountify](https://github.com/backslashxx/mountify); vedi [`module/lkm/README.md`](../module/lkm/README.md) e [`module/lkm/src/LICENSE`](../module/lkm/src/LICENSE).
- Sottosistema VFS (modulo `hybridmount`): GPL-2.0-only, derivato da [NoMount](https://github.com/maxsteeel/nomount); vedi [`module/vfs/README.md`](../module/vfs/README.md), [`module/vfs/src/LICENSE`](../module/vfs/src/LICENSE) e [THIRD_PARTY.md](../THIRD_PARTY.md).
