# Hybrid Mount

Hybrid Mount est un méta-module de montage hybride pour KernelSU et APatch. Au démarrage, il analyse les autres modules et sélectionne OverlayFS, Magic Mount ou l'ignorance de chaque élément selon les règles globales, celles du module et celles du chemin. Les répertoires sources des modules sont toujours traités comme des entrées en lecture seule.

## Fonctionnalités

- OverlayFS et Magic Mount peuvent être combinés par module et par chemin.
- Les règles de chemin sont prioritaires sur les valeurs par défaut du module, elles-mêmes prioritaires sur la valeur globale par défaut.
- OverlayFS prend en charge les modes de stockage tmpfs et ext4.
- Pour la zone de préparation ext4, KernelSU utilise l'ioctl officiel afin de masquer les nœuds sysfs ; APatch et les autres environnements sans KSU utilisent par défaut le LKM de compatibilité fourni.
- Magic Mount prend en charge les fichiers, les répertoires, les liens symboliques, `.replace` et la sémantique whiteout.
- La WebUI propose les interfaces MD3 (par défaut) et Miuix.
- Les architectures arm64, armv7 et x86_64 sont prises en charge ; le programme d'installation sélectionne automatiquement le binaire correspondant.

## Installation

Téléchargez le fichier ZIP depuis la page [Releases](https://github.com/Hybrid-Mount/meta-hybrid_mount/releases), puis installez-le avec le gestionnaire KernelSU ou APatch. Lors de la première installation, utilisez les touches de volume pour sélectionner le backend par défaut. Les mises à niveau conservent `/data/adb/hybrid-mount/config.toml`.

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

Les chemins des règles sont relatifs à la racine du module. Les règles au niveau du module et du chemin peuvent également utiliser `ignore` ; le backend global par défaut accepte uniquement `overlay` ou `magic`. Un même chemin de fichier ne peut pas être attribué aux deux backends de montage. Les répertoires ordinaires peuvent servir de nœuds structurels communs aux deux backends, tandis que les conflits de fichier, de type ou de `.replace` provoquent l'échec immédiat de l'étape de planification du démarrage. Les modifications de configuration prennent effet après le redémarrage.

Ce routage ne modifie pas la vérification existante de la capacité `CONFIG_TMPFS_XATTR`. Avec KernelSU, l'installation supprime l'intégralité du répertoire `lkm/` du module et l'exécution utilise uniquement l'ioctl officiel `NukeExt4Sysfs`. Les installations APatch et les autres environnements sans KSU conservent le LKM et tentent de l'utiliser par défaut après le montage de la zone de préparation ext4. Les fichiers `.ko` fournis prennent uniquement en charge aarch64. La sélection automatique exige une correspondance exacte entre la branche du noyau et l'étiquette Android/GKI ; les combinaisons inconnues sont rejetées. La compatibilité ABI des LKM précompilés doit tout de même être validée sur l'appareil physique correspondant. Si l'appareil plante pendant `insmod`, un marqueur coupe-circuit persistant empêche le chargement du LKM au démarrage suivant tout en préservant le reste des fonctionnalités de Hybrid Mount. Consultez [`module/lkm/README.md`](../module/lkm/README.md) pour la matrice de prise en charge, les sommes de contrôle, les sources et les licences.

## Retours

Avant l'installation ou le signalement d'un problème, lisez les [consignes d'utilisation](../USAGE_NOTICE.md). Joignez le rapport de bogue KernelSU/APatch, la version du module et les étapes de reproduction. Contactez-nous via les [issues GitHub](https://github.com/Hybrid-Mount/meta-hybrid_mount/issues) ou le [groupe Telegram](https://t.me/hybridmountchat).

## Langues / Languages

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

## Licence

- Cœur (Rust et scripts du module) : GPL-3.0-only (voir [`LICENSE`](../LICENSE)).
- WebUI : Apache-2.0 (voir [`webui/LICENSE`](../webui/LICENSE)).
- LKM sysfs ext4 facultatif (sources et fichiers `.ko` précompilés) : GPL-2.0-only, dérivé de [Mountify](https://github.com/backslashxx/mountify) ; voir [`module/lkm/README.md`](../module/lkm/README.md) et [`module/lkm/src/LICENSE`](../module/lkm/src/LICENSE).
