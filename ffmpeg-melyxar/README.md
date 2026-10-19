# ffmpeg-melyxar

Le FFmpeg que Melyxar fait tourner : la dernière version stable officielle de FFmpeg, avec nos propres corrections. Ce dossier ne contient pas FFmpeg, seulement ce qui le distingue de l'officiel.

| Fichier | Rôle |
|---|---|
| `release` | La version officielle de départ, l'empreinte SHA-256 de son archive, et notre numéro de révision. |
| `patches/` | Nos corrections, appliquées dans l'ordre de leur numéro. Chacune commence par ce qu'elle corrige et pourquoi. |
| `configure-options` | Les options de compilation : seulement ce dont Melyxar se sert. |
| `packages` | Les paquets Debian nécessaires à la compilation, un par bibliothèque activée. |
| `build.sh` | Télécharge l'archive, vérifie son empreinte, applique les corrections, compile `ffmpeg` et `ffprobe`. |

Le script d'installation et de mise à jour lance `build.sh` et pose le résultat dans `/opt/melyxar/ffmpeg/bin`. Il ne recompile que si quelque chose a changé dans ce dossier. Le serveur prend ce FFmpeg en premier, puis celui de Debian en secours (voir le README des décisions).

## Passer à une nouvelle version de FFmpeg

1. Dans `release` : la nouvelle version, l'empreinte SHA-256 de son `.tar.xz` publiée sur ffmpeg.org, et `MELYXAR_REVISION` remis à 1.
2. Vérifier que chaque correction s'applique encore (`build.sh` s'arrête sinon). Une correction reprise dans l'officiel est supprimée ; une correction qui ne s'applique plus est refaite sur le nouveau code.
3. Vérifier que les bibliothèques demandées par `configure-options` existent encore dans Debian aux versions que FFmpeg exige.

## Ajouter une correction

Un nouveau fichier dans `patches/`, numéroté à la suite, au format `diff -u` appliqué depuis la racine des sources de FFmpeg (`patch -p1`), avec en tête l'explication de ce qu'il corrige. Puis `MELYXAR_REVISION` augmenté de 1.

## Ce qui manque volontairement

`libplacebo` : FFmpeg 9 en demande une version plus récente que celle de Debian 13. Une carte qui ne sait convertir le HDR que par lui le voit à ses essais de démarrage, et ses films HDR passent au processeur.
