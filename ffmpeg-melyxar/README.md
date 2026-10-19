# ffmpeg-melyxar

Le FFmpeg que Melyxar fait tourner : la dernière version stable officielle de FFmpeg, avec nos propres corrections. Ce dossier ne contient pas FFmpeg, seulement ce qui le distingue de l'officiel.

| Fichier | Rôle |
|---|---|
| `release` | La version officielle de départ, l'empreinte SHA-256 de son archive, et notre numéro de révision. |
| `patches/` | Nos corrections, appliquées dans l'ordre de leur numéro. Chacune commence par ce qu'elle corrige et pourquoi. |
| `configure-options` | Les options de compilation : seulement ce dont Melyxar se sert. |
| `packages` | Les paquets Debian nécessaires à la compilation, un par bibliothèque activée. |
| `build.sh` | Compile libplacebo à la version de `release`, puis télécharge l'archive de FFmpeg, vérifie son empreinte, applique les corrections et compile `ffmpeg` et `ffprobe`. |

Le script d'installation et de mise à jour lance `build.sh` et pose le résultat dans `/opt/melyxar/ffmpeg` (`bin` pour les outils, `lib` pour libplacebo, que les outils trouvent à côté d'eux). Il ne recompile que si quelque chose a changé dans ce dossier. Le serveur prend ce FFmpeg en premier, puis celui de Debian en secours (voir le README des décisions).

## Passer à une nouvelle version de FFmpeg

1. Dans `release` : la nouvelle version, l'empreinte SHA-256 de son `.tar.xz` publiée sur ffmpeg.org, et `MELYXAR_REVISION` remis à 1. Pour libplacebo, la nouvelle étiquette et le commit qu'elle désigne.
2. Vérifier que chaque correction s'applique encore (`build.sh` s'arrête sinon). Une correction reprise dans l'officiel est supprimée ; une correction qui ne s'applique plus est refaite sur le nouveau code.
3. Vérifier que les bibliothèques demandées par `configure-options` existent encore dans Debian aux versions que FFmpeg exige.

## Ajouter une correction

Un nouveau fichier dans `patches/`, numéroté à la suite, au format `diff -u` appliqué depuis la racine des sources de FFmpeg (`patch -p1`), avec en tête l'explication de ce qu'il corrige. Puis `MELYXAR_REVISION` augmenté de 1.

## Ce qui reste à corriger

Melyxar doit tout faire sur la carte, qu'elle soit Intel, AMD ou NVIDIA, et pas seulement sur le matériel du mainteneur. Ce que FFmpeg officiel empêche encore, relevé dans le code du serveur, du plus utile au plus lourd :

1. **`overlay_vulkan` écrivait des heures mille fois trop grandes** : corrigé (correction 0002), vérifié sur la carte AMD : un film HDR avec sous-titres en images se lit, couleurs justes.
2. **Le passage d'une image de Vulkan à l'encodeur VAAPI** faisait un aller-retour complet carte, processeur, carte : corrigé par trois corrections reprises de Jellyfin (0003 à 0005), et le serveur essaie maintenant de garder l'image sur la carte, en vérifiant ses couleurs. L'image verte venait d'ailleurs : Vulkan rend mal une image à deux couches, le serveur lui en demande désormais une seule. Sur la RX 6600, les films réécrits en HEVC sortaient en une bande de couleurs éparpillées au-dessus du noir : l'encodeur HEVC d'AMD travaille avec d'autres réglages que ceux annoncés dans ses en-têtes (correction 0006, reprise de Jellyfin). Ensuite, seules les images clés sortaient justes et celles d'entre elles en blocs, dans le navigateur comme sur la SHIELD : FFmpeg retirait du flux la description que l'encodeur écrit avant chaque image clé et ne gardait que celle d'en tête, inexacte chez AMD (correction 0007, même origine). Vérifié sur la RX 6600 : image juste et fluide, dans le navigateur comme sur la SHIELD.
3. **Aucune conversion HDR propre aux cartes NVIDIA** : elles passent par Vulkan, avec un aller-retour de l'image et parfois une lecture par le processeur, faute de pouvoir passer une image du lecteur NVIDIA à Vulkan. Jellyfin a son filtre `tonemap_cuda` (environ 2 700 lignes).
4. **Le découpeur en morceaux de FFmpeg 7.1** refusait une image recopiée plus longue que l'écart réel ; le serveur corrige la durée de chaque image (`setts`). À vérifier sur 9.0.2 avant de retirer.
5. **Taille visible d'un HEVC écrit par VAAPI** (`hevc_metadata`) : corrigé dans FFmpeg depuis la 8.0, le contournement du serveur peut partir.

Ce qui ne relève pas de FFmpeg et qu'une correction ici ne changerait pas : les limites des pilotes (un pilote AMD sans `tonemap_vaapi`, une carte qui refuse une taille), celles des navigateurs (un sous-titre en images ne peut être qu'incrusté), et le réveil lent des cartes AMD.
