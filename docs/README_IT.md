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

## Backend VFS

VFS è il percorso di iniezione lato kernel di Hybrid Mount, pilotato tramite il keyring dal modulo `hybridmount`. È un'implementazione indipendente e non interopera con NoMount.

**Come viene identificato il provider.** La decisione di avvio si basa esclusivamente su una sonda in sola lettura del tipo di chiave del kernel `hybridmount`: se risponde con una versione supportata, il provider è utilizzabile. Separatamente, `vfs-doctor` classifica come è presente: una voce in `/proc/modules` significa che l'ha registrato un modulo caricabile; una directory `/sys/module/hybridmount` senza quella voce significa che è compilato nell'immagine del kernel; se non esiste nessuno dei due, non è presente alcun provider. La sonda è in sola lettura, quindi `status` e `vfs-doctor` non attivano mai un `insmod`.

**Logica di avvio.** Se il tipo di chiave risponde con una versione supportata, il provider viene associato e non viene caricato nulla. Se nessuna regola seleziona VFS, non viene caricato nemmeno il modulo incluso. Se una regola seleziona VFS mentre la sonda resta muta, la pipeline di avvio sceglie il modulo incluso che corrisponde esattamente alla linea del kernel e al tag Android/GKI, lo carica e sonda di nuovo; se resta non disponibile, ogni regola `vfs` viene degradata a `ignore`, oppure l'avvio fallisce quando `vfs_strict = true`. Il caricamento avviene prima della costruzione del piano di mount, perché la pianificazione riscrive le regole `vfs` in `ignore` finché il provider è muto e l'esecutore tornerebbe quindi in anticipo. Un indicatore di protezione viene scritto prima di `insmod` e rimosso quando il tentativo ritorna, quindi solo un crash del kernel lo lascia dietro di sé; l'avvio successivo rifiuta allora un nuovo tentativo automatico finché l'indicatore non viene rimosso manualmente.

**Integrare VFS in un kernel.** Le release includono un modulo aarch64 precompilato per ogni target Android/GKI supportato e lo caricano automaticamente, quindi quei kernel non richiedono alcun passaggio di integrazione. Compilalo nel kernel quando vuoi evitare l'`insmod`, o quando la tua linea del kernel non ha un precompilato. Dalla radice di un albero del kernel:

```sh
sh /path/to/metamodule/module/vfs/setup.sh
```

Questo copia i sorgenti in `fs/hybridmount/` e li aggiunge a `fs/Makefile` e `fs/Kconfig`; abilita `CONFIG_HYBRIDMOUNT=y` per compilarlo nel kernel oppure `=m` per compilarlo come modulo. `--cleanup` annulla tutte le modifiche. Un albero che integra già NoMount viene rifiutato: entrambe le implementazioni dirottano le operazioni sugli inode e il kernel non impedirà che coesistano, dato che registrano tipi di chiave diversi.

**Diagnosi.** `/data/adb/modules/hybrid_mount/hybrid-mount vfs-doctor` riporta lo stato di presenza, la versione con cui ha risposto il tipo di chiave, le versioni supportate e, quando un provider non è utilizzabile, il motivo.

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

## Ringraziamenti

- Grazie a [Anatdx](https://github.com/Anatdx)
- Grazie a [Tools-cx-app](https://github.com/Tools-cx-app)
- Grazie a [KernelSU](https://github.com/tiann/KernelSU)
- Grazie a [MKSU di 5ec1cff](https://github.com/5ec1cff/KernelSU)
- Grazie a [ReSukiSU](https://github.com/ReSukiSU/ReSukiSU)
- Grazie a [meta-magic_mount-rs](https://github.com/Tools-cx-app/meta-magic_mount-rs)
- Grazie a [NoMount](https://github.com/maxsteeel/nomount)

## Licenza

- Core (Rust e script del modulo): GPL-3.0-only (vedi [`LICENSE`](../LICENSE)).
- WebUI: Apache-2.0 (vedi [`webui/LICENSE`](../webui/LICENSE)).
- LKM sysfs ext4 opzionale (sorgenti e file `.ko` precompilati): GPL-2.0-only, derivato da [Mountify](https://github.com/backslashxx/mountify); vedi [`module/lkm/README.md`](../module/lkm/README.md) e [`module/lkm/src/LICENSE`](../module/lkm/src/LICENSE).
- Sottosistema VFS (modulo `hybridmount`): GPL-2.0-only, derivato da [NoMount](https://github.com/maxsteeel/nomount); vedi [`module/vfs/README.md`](../module/vfs/README.md), [`module/vfs/src/LICENSE`](../module/vfs/src/LICENSE) e [THIRD_PARTY.md](../THIRD_PARTY.md).
