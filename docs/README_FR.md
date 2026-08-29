# Hybrid Mount

Hybrid Mount est un méta-module de montage hybride pour KernelSU et APatch. Au démarrage, il analyse les autres modules et sélectionne pour chaque module une méhode de montage OverlayFS ou Magic Mount, ou l'ignore selon des règles globales, spécifiques au module et/ou spécifiques à un chemin de fichier. 
Les répertoires sources des modules sont toujours traités comme des entrées en lecture seule.

## Fonctionnalités

- OverlayFS et Magic Mount peuvent être combinés par module et par chemin.
- Les règles par chemin sont prioritaires aux règles par défaut des modules, elles-mêmes prioritaires aux règles globales par défaut.
- OverlayFS prend en charge les modes de stockage tmpfs et ext4.
- Pour la zone tampon ext4, KernelSU utilise des appels ioctl officiels afin de masquer les nœuds sysfs ; APatch et les autres environnements sans KSU utilisent par défaut le mode de compatibilité de LKM fourni.
- Magic Mount prend en charge les fichiers, les répertoires, les liens symboliques, `.replace` et la sémantique whiteout.
- La WebUI propose un thème d'affichage Material Design 3 (par défaut) ou Miuix.
- Les architectures arm64, armv7 et x86_64 sont prises en charge ; le programme d'installation sélectionne automatiquement le binaire correspondant.

## Installation

Télécharger le fichier ZIP depuis la page [Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases), puis l'installer avec le gestionnaire KernelSU ou APatch. 
Lors de la première installation, utiliser les touches de volume pour sélectionner le backend par défaut. Les mises à niveau conservent la configuration dans le fichier `/data/adb/hybrid-mount/config.toml`.

## Configuration

Configuration par défaut :

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

Les chemins des règles sont relatifs à la racine du module. Les règles de module et de chemin de fichier peuvent également utiliser l'option `ignore` ; le backend global par défaut accepte uniquement `overlay` ou `magic`. Un même chemin de fichier ne peut pas être attribué aux deux backends de montage. Les répertoires ordinaires peuvent servir de nœuds structurels communs aux deux backends, tandis que les conflits de fichier, de type ou de `.replace` provoqueront l'échec immédiat de l'étape de planification du démarrage. Les modifications de configuration prennent effet après redémarrage.

Ce routage ne modifie pas la vérification de l'existence de la fonctionnalité `CONFIG_TMPFS_XATTR`. Avec KernelSU, l'installation supprime l'intégralité du répertoire `lkm/` du module et l'exécution utilise uniquement la fonction ioctl officielle `NukeExt4Sysfs`. Les installations sur APatch et les autres environnements sans KSU conservent le LKM et tentent de l'utiliser par défaut après le montage de la zone tampon ext4. Les fichiers `.ko` fournis prennent uniquement en charge l'architecture aarch64. La sélection automatique exige une correspondance exacte entre la branche du noyau et l'étiquette Android/GKI ; les combinaisons inconnues sont rejetées. La compatibilité ABI des LKM précompilés doit tout de même être validée sur l'appareil physique correspondant. Si l'appareil plante pendant `insmod`, un marqueur coupe-circuit persistant empêchera le chargement du LKM au démarrage suivant tout en préservant le reste des fonctionnalités de Hybrid Mount. Consulter [`module/lkm/README.md`](../module/lkm/README.md) pour la matrice de compatibilité, les sommes de contrôle, les sources et les licences.

## Retours

Avant l'installation ou le signalement d'un problème, lire les [consignes d'utilisation](../USAGE_NOTICE.md). Joindre le rapport de bugs de KernelSU/APatch, la version du module et les étapes permettant de reproduire le problème. Nous contacter : via les [issues GitHub](https://github.com/Hybrid-Mount/meta-hybrid_mount/issues) ou le [groupe Telegram](https://t.me/hybridmountchat).

## Langues

- [English](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_EN.md)
- [Español](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_ES.md)
- [Français](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_FR.md)
- [Bahasa Indonesia](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_ID.md)
- [Italiano](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_IT.md)
- [日本語](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_JA.md)
- [Русский](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_RU.md)
- [Türkçe](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_TR.md)
- [Українська](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_UK.md)
- [Tiếng Việt](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_VI.md)
- [简体中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/README.md)
- [繁體中文](https://github.com/Hybrid-Mount/meta-hybrid_mount/blob/dev/docs/README_ZH_TW.md)

## Licences

- Code principal (Rust et scripts du module) : GPL-3.0-only (consulter [`LICENSE`](../LICENSE)).
- WebUI : Apache-2.0 (consulter [`webui/LICENSE`](../webui/LICENSE)).
- LKM sysfs ext4 facultatif (sources et fichiers `.ko` précompilés) : GPL-2.0-only, dérivé de [Mountify](https://github.com/backslashxx/mountify) ; consulter [`module/lkm/README.md`](../module/lkm/README.md) et [`module/lkm/src/LICENSE`](../module/lkm/src/LICENSE).
