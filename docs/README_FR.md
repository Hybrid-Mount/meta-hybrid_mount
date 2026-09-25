# Hybrid Mount

<img src="../icon.svg" alt="Hybrid Mount logo" align="right" width="120" />

Hybrid Mount est un méta-module de montage hybride pour KernelSU et APatch. Au démarrage, il analyse les autres modules et sélectionne OverlayFS, Magic Mount, VFS ou l'ignorance selon les règles globales, du module et du chemin.
Les répertoires sources des modules sont toujours traités comme des entrées en lecture seule.

## Fonctionnalités

- OverlayFS, Magic Mount et VFS peuvent être combinés par module et par chemin.
- Les règles par chemin sont prioritaires aux règles par défaut des modules, elles-mêmes prioritaires aux règles globales par défaut.
- OverlayFS prend en charge les modes de stockage tmpfs et ext4.
- Pour la zone tampon ext4, KernelSU utilise des appels ioctl officiels afin de masquer les nœuds sysfs ; APatch et les autres environnements sans KSU utilisent par défaut le mode de compatibilité de LKM fourni.
- Magic Mount prend en charge les fichiers, les répertoires, les liens symboliques, `.replace` et la sémantique whiteout.
- VFS envoie les règles d'injection au sous-système VFS propre à Hybrid Mount (module `hybridmount`) via le keyring. C'est une implémentation indépendante qui n'interopère ni avec le noyau de NoMount ni avec sa CLI nm. Les versions incluent les sources et un module arm64 précompilé par cible Android/GKI prise en charge, chargé au démarrage lorsque le noyau ne l'intègre pas ; en cas d'échec, le comportement suit `vfs_strict`. VFS n'est pas un montage réel.
- La WebUI propose un thème d'affichage Material Design 3 (par défaut) ou Miuix.
- Les architectures arm64, armv7, x86_64 et riscv64 sont prises en charge ; le programme d'installation sélectionne automatiquement le binaire correspondant. La compilation riscv64 nécessite le NDK Android r27 ou une version ultérieure.

## Installation

Télécharger le fichier ZIP depuis la page [Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases), puis l'installer avec le gestionnaire KernelSU ou APatch. 
Lors de la première installation, utiliser les touches de volume pour sélectionner le backend par défaut. Les mises à niveau conservent la configuration dans le fichier `/data/adb/hybrid-mount/config.toml`.

## Configuration

Configuration par défaut :

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

Les chemins des règles sont relatifs à la racine du module. Les règles de module et de chemin peuvent aussi utiliser `ignore` ; le backend global par défaut accepte `overlay`, `magic` ou `vfs`. VFS est un chemin d'injection, pas un montage réel. Les conflits de fichier, de type ou de `.replace` provoquent l'échec immédiat de la planification du démarrage. Les modifications prennent effet après redémarrage.

Ce routage ne modifie pas la vérification de l'existence de la fonctionnalité `CONFIG_TMPFS_XATTR`. Avec KernelSU, l'installation supprime l'intégralité du répertoire `lkm/` du module et l'exécution utilise uniquement la fonction ioctl officielle `NukeExt4Sysfs`. Les installations sur APatch et les autres environnements sans KSU conservent le LKM et tentent de l'utiliser par défaut après le montage de la zone tampon ext4. Les fichiers `.ko` fournis prennent uniquement en charge l'architecture aarch64. La sélection automatique exige une correspondance exacte entre la branche du noyau et l'étiquette Android/GKI ; les combinaisons inconnues sont rejetées. La compatibilité ABI des LKM précompilés doit tout de même être validée sur l'appareil physique correspondant. Si l'appareil plante pendant `insmod`, un marqueur coupe-circuit persistant empêchera le chargement du LKM au démarrage suivant tout en préservant le reste des fonctionnalités de Hybrid Mount. Consulter [`module/lkm/README.md`](../module/lkm/README.md) pour la matrice de compatibilité, les sommes de contrôle, les sources et les licences.

## Backend VFS

VFS est le chemin d'injection côté noyau propre à Hybrid Mount, piloté via le keyring par le module `hybridmount`. C'est une implémentation indépendante qui n'interopère pas avec NoMount.

**Comment le fournisseur est identifié.** La décision de démarrage repose uniquement sur une sonde en lecture seule du type de clé `hybridmount` : s'il répond avec une version prise en charge, le fournisseur est utilisable. Séparément, `vfs-doctor` classe la façon dont il est présent : une entrée dans `/proc/modules` signifie qu'un module chargeable l'a enregistré ; un répertoire `/sys/module/hybridmount` sans cette entrée signifie qu'il est compilé dans l'image du noyau ; si ni l'un ni l'autre n'existe, aucun fournisseur n'est présent. La sonde est en lecture seule, donc `status` et `vfs-doctor` ne déclenchent jamais d'`insmod`.

**Logique de démarrage.** Si le type de clé répond avec une version prise en charge, le fournisseur est lié et rien n'est chargé. Si aucune règle ne sélectionne VFS, le module fourni n'est pas chargé non plus. Si une règle sélectionne VFS alors que la sonde reste muette, la chaîne de démarrage choisit le module fourni correspondant exactement à la branche du noyau et à l'étiquette Android/GKI, le charge, puis sonde à nouveau ; s'il reste indisponible, chaque règle `vfs` est dégradée en `ignore`, ou fait échouer le démarrage lorsque `vfs_strict = true`. Le chargement s'exécute avant la construction du plan de montage, car la planification réécrit les règles `vfs` en `ignore` tant que le fournisseur est muet et l'exécuteur retournerait alors prématurément. Un marqueur coupe-circuit est écrit avant `insmod` et effacé lorsque la tentative se termine, de sorte que seul un plantage du noyau le laisse derrière lui ; le démarrage suivant refuse alors toute nouvelle tentative automatique tant que le marqueur n'est pas supprimé manuellement.

**Intégrer VFS dans un noyau.** Les versions incluent un module aarch64 précompilé pour chaque cible Android/GKI prise en charge et le chargent automatiquement, ces noyaux n'ont donc besoin d'aucune étape d'intégration. Intégrez-le à la compilation pour éviter l'`insmod`, ou lorsque votre branche de noyau n'a pas de précompilé. Depuis la racine d'une arborescence de noyau :

```sh
curl -LSs "https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/dev/module/vfs/setup.sh" | bash
```

`--cleanup`:

```sh
curl -LSs "https://raw.githubusercontent.com/Hybrid-Mount/meta-hybrid_mount/dev/module/vfs/setup.sh" | bash -s -- --cleanup
```

Cela copie les sources dans `fs/hybridmount/` et les ajoute à `fs/Makefile` et `fs/Kconfig` ; activez `CONFIG_HYBRIDMOUNT=y` pour l'intégrer à la compilation ou `=m` pour le compiler comme module. `bash -s -- --cleanup` annule toutes les modifications. Une arborescence qui intègre déjà NoMount est refusée : les deux implémentations détournent les opérations d'inode et le noyau ne les empêchera pas de coexister, puisqu'elles enregistrent des types de clé différents.

**Diagnostic.** `/data/adb/modules/hybrid_mount/hybrid-mount vfs-doctor` indique l'état de présence, la version à laquelle le type de clé a répondu, les versions prises en charge et, lorsqu'un fournisseur est inutilisable, la raison.

## Retours

Avant l'installation ou le signalement d'un problème, lire les [consignes d'utilisation](../USAGE_NOTICE.md). Joindre le rapport de bugs de KernelSU/APatch, la version du module et les étapes permettant de reproduire le problème. Nous contacter : via les [issues GitHub](https://github.com/Hybrid-Mount/meta-hybrid_mount/issues) ou le [groupe Telegram](https://t.me/hybridmountchat).

## Langues

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

## Remerciements

- Merci à [Anatdx](https://github.com/Anatdx)
- Merci à [Tools-cx-app](https://github.com/Tools-cx-app)
- Merci à [KernelSU](https://github.com/tiann/KernelSU)
- Merci à [MKSU de 5ec1cff](https://github.com/5ec1cff/KernelSU)
- Merci à [ReSukiSU](https://github.com/ReSukiSU/ReSukiSU)
- Merci à [meta-magic_mount-rs](https://github.com/Tools-cx-app/meta-magic_mount-rs)
- Merci à [NoMount](https://github.com/maxsteeel/nomount)

## Licences

- Code principal (Rust et scripts du module) : GPL-3.0-only (consulter [`LICENSE`](../LICENSE)).
- WebUI : Apache-2.0 (consulter [`webui/LICENSE`](../webui/LICENSE)).
- LKM sysfs ext4 facultatif (sources et fichiers `.ko` précompilés) : GPL-2.0-only, dérivé de [Mountify](https://github.com/backslashxx/mountify) ; consulter [`module/lkm/README.md`](../module/lkm/README.md) et [`module/lkm/src/LICENSE`](../module/lkm/src/LICENSE).
- Sous-système VFS (module `hybridmount`) : GPL-2.0-only, dérivé de [NoMount](https://github.com/maxsteeel/nomount) ; consulter [`module/vfs/README.md`](../module/vfs/README.md), [`module/vfs/src/LICENSE`](../module/vfs/src/LICENSE) et [THIRD_PARTY.md](../THIRD_PARTY.md).
