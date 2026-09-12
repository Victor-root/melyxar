# Architecture de Melyxar

Ce dossier est la mémoire du projet. Il contient les revues d'architecture réalisées avant la première ligne de code, et les décisions actées. Tout ce qui est décidé ici doit être respecté par le code, et toute nouvelle décision doit y être ajoutée.

## Documents

| Fichier | Contenu |
|---|---|
| [01-revue-architecture.md](01-revue-architecture.md) | Revue complète des choix techniques (Rust, Axum, base de données, SQLx, FFmpeg, HLS, hls.js, React, monolithe modulaire), architecture révisée en crates, modèle de données, moteur de lecture, abstractions à figer, erreurs classiques, sujets à ne pas oublier, V0.1 révisée. |
| [02-reactivite.md](02-reactivite.md) | Exigence de navigation quasi instantanée jusqu'à 100 000 médias : analyse Jellyfin contre Emby, principe chemin chaud / chemin froid, stratégie serveur et client, isolation des tâches de fond, ce qui est à faire dès le début ou prématuré, mesure et détection des régressions, outils de diagnostic. |

## Décisions actées

Ces décisions ont été prises après discussion et ne sont pas à rediscuter sans raison nouvelle.

| Décision | Choix | Pourquoi |
|---|---|---|
| Langage serveur | Rust, Tokio, Axum | Superviseur de processus, faible consommation, binaire unique. |
| Base de données | **SQLite** (mode WAL, une seule connexion d'écriture, transactions courtes) | Un seul binaire, sauvegarde par copie de fichier, plus rapide qu'un aller-retour réseau pour ce type d'usage, Emby et Plex prouvent l'échelle. PostgreSQL écarté définitivement. |
| Accès base | SQLx, métadonnées de requêtes versionnées dans le dépôt (mode hors ligne), SQL uniquement dans la crate `database` | Vérification des requêtes à la compilation sans dépendre de l'état de la base. |
| FFmpeg | Programme externe, jamais les bibliothèques `libav*`, chemin configurable, une seule crate `ffmpeg` lance des binaires | Robustesse, mise à jour indépendante, accélération matérielle simple. |
| Streaming | HLS en fragments fMP4, playlist générée par le serveur, segments à la demande | Seeking instantané, un seul pipeline pour remux et transcodage. |
| Client web | React, TypeScript, Vite, hls.js, client TypeScript généré depuis la spécification OpenAPI | Écosystème d'interface le plus riche ; le frontend ne connaît que l'API. |
| Structure | Monolithe modulaire, workspace Cargo, direction unique des dépendances | Frontières imposées par le compilateur. |
| Modèle de données | Œuvre, source média et piste sont trois entités distinctes ; identifiants internes UUID v7 ; identifiants externes dans une table à part ; chemins relatifs à une racine ; millisecondes et UTC partout | Remplacer un fichier ne doit jamais effacer l'historique. |
| Utilisateurs | Un utilisateur par défaut et un jeton de session dès la V0.1 | Toute donnée de progression est rattachée à un utilisateur dès le départ. |
| Réactivité | Rien de lourd sur le chemin de lecture : tout est précalculé à l'écriture | Exigence forte, voir le document 02. |
| Compilation | **Directement dans le LXC de production**, via une commande unique de mise à jour (récupérer, compiler en priorité basse, migrer, redémarrer) | Les PC du mainteneur sont sous Windows ; aucun transfert de fichier. |
| Ressources du LXC | 100 Go de disque, 16 Go de mémoire, 12 threads (monter à 16 si les autres services de l'hôte le permettent) | Le 16 Go et les threads servent surtout à la compilation ; le serveur lui-même est léger. |
| Matériel de transcodage | Intel Arc A380 passée au LXC via `/dev/dri`, distribution FFmpeg de Jellyfin recommandée | Décodage, encodage H.264, HEVC, AV1 et tonemapping HDR sur la carte. |

## Environnement de production

- Hôte Proxmox : Ryzen 7 3700X (8 cœurs, 16 threads), 32 Go DDR4, Intel Arc A380, NVMe.
- LXC Melyxar : Linux, non privilégié, 12 threads, 16 Go, 100 Go sur NVMe.
- Première cible client : Brave / Chromium sous Windows, en réseau local.
- Le mainteneur ne lit pas le code : les journaux, la commande de diagnostic et la page de diagnostic sont conçus pour lui.

## Règles de travail

- Le em dash est banni de tout texte du projet.
- Textes d'interface toujours passés par l'internationalisation, anglais d'abord, français ensuite.
- Tout journal de débogage est protégé par un contrôle de type `BuildConfig.DEBUG` ou équivalent Rust (`cfg(debug_assertions)`).
- Pas de code mort, pas de contournement temporaire, pas de commentaire inutile.
