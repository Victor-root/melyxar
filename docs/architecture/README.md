# Architecture de Melyxar

Ce dossier est la mémoire du projet. Il contient les revues d'architecture réalisées avant la première ligne de code, et les décisions actées. Tout ce qui est décidé ici doit être respecté par le code, et toute nouvelle décision doit y être ajoutée.

## Documents

| Fichier | Contenu |
|---|---|
| [01-revue-architecture.md](01-revue-architecture.md) | Revue complète des choix techniques (Rust, Axum, base de données, SQLx, FFmpeg, HLS, hls.js, React, monolithe modulaire), architecture révisée en crates, modèle de données, moteur de lecture, abstractions à figer, erreurs classiques, sujets à ne pas oublier, V0.1 révisée. |
| [02-reactivite.md](02-reactivite.md) | Exigence de navigation quasi instantanée jusqu'à 100 000 médias : analyse Jellyfin contre Emby, principe chemin chaud / chemin froid, stratégie serveur et client, isolation des tâches de fond, ce qui est à faire dès le début ou prématuré, mesure et détection des régressions, outils de diagnostic. |
| [03-plan.md](03-plan.md) | Plan par jalons de la V0.1 et état d'avancement. |
| [04-fonctionnalites.md](04-fonctionnalites.md) | Ce que Melyxar doit savoir faire, demande par demande, avec les conséquences sur l'architecture. Se remplit au fil des discussions. |

Le fichier `CLAUDE.md` à la racine du dépôt résume les règles pour chaque session de travail.

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
| Modèle multi-domaines | La bibliothèque a un type explicite (films, séries, musique) ; le tronc commun (source, piste, progression, favoris, images) est séparé des métadonnées propres à chaque domaine | La musique est prévue à terme ; un schéma qui suppose « un film » imposerait une refonte. |
| Sonie audio | Colonnes EBU R128 sur les pistes audio dès la première migration, mesurées au scan, appliquées au gain à la lecture | Sans la colonne, toute la bibliothèque devrait être réanalysée plus tard. |
| Progression | État explicite (non commencé, en cours, vu), date de lecture, marquage manuel prioritaire, position horodatée jamais écrasée par une plus ancienne, copie locale côté client | Déduire « vu » d'un pourcentage empêche le marquage manuel ; sans horodatage, la reprise peut reculer. |
| Compteurs | Nombre d'épisodes et d'épisodes non vus stockés par utilisateur et par saison ou série, mis à jour à l'écriture | Les compter à la lecture ruine la réactivité. |
| Personnalisation | Paramètres du serveur (nom, logo, fonds, CSS global) et préférences utilisateur (thème, couleur, langue, volume, CSS personnel) dans des tables dédiées ; fichiers envoyés dans les données ; route publique d'identité visuelle sans divulgation | La page de connexion doit afficher le logo avant toute authentification. |
| Thème | Aucune couleur, aucun espacement, aucun rayon en dur dans le frontend : tout par jetons. Modes clair, sombre et automatique. CSS personnalisé désactivable avec adresse de secours | Ajouter un thème après coup oblige à repasser sur chaque composant ; un CSS cassé ne doit jamais bloquer l'accès. |
| Erreurs de l'API | Code d'erreur et données structurées, jamais de phrase toute faite | Sans cela, les clients ne peuvent pas traduire les messages. |
| Chiffrement | Le serveur sait servir en HTTPS lui-même ; quatre options proposées (reverse proxy, auto-signé, certificat fourni, certificat reconnu par nom de domaine) avec leurs limites annoncées honnêtement | Un certificat auto-signé ne supprime pas l'avertissement du navigateur ; le promettre serait mentir. |
| Clients natifs | Un seul projet Android, base commune (API, session, cache, lecteur), interfaces séparées, télévision d'abord | Deux applications distinctes coûtent cher en doublons ; une interface unique est médiocre partout. |
| Personnes | Table des personnes unique et table de participation (rôle, personnage, ordre) | Sans identité unique, la filmographie d'un acteur est impossible. |
| Collections | Une seule table pour les sagas automatiques et manuelles, avec l'origine indiquée | Sinon un rafraîchissement des métadonnées efface les collections créées à la main. |
| Droits par utilisateur | Accès par bibliothèque, limite d'âge, téléchargement, suppression, sessions simultanées | Chaque fonction sensible s'y rattache ; les ajouter après multiplie les migrations. |
| Images extraites d'un fichier | Toujours converties en SDR quand la source est HDR, par défaut | Sans conversion, les vignettes d'un film HDR sont délavées et grisâtres. |
| Aperçu de la barre de lecture | Vignettes regroupées en planches, intervalle et résolution configurables, activable par bibliothèque | Des fichiers individuels donneraient des centaines de milliers de petits fichiers. |
| Segments repérés | Table des segments (récapitulatif, générique de début, générique de fin) sur la source média | Base du saut d'intro et du bouton « épisode suivant ». |
| Suppression sur disque | Jamais de chemin venant du client, chemin résolu et vérifié sous une racine déclarée, case décochée par défaut, droit d'administrateur, entrée au journal | C'est l'opération la plus dangereuse du projet. |
| Métadonnées multilingues | Textes stockés par langue dans une table de traductions, français prioritaire, repli anglais | Des colonnes de texte sur l'œuvre interdiraient le repli et le changement de langue. |
| Versions multiples | Sélecteur de version sur la fiche, décrit par résolution, codec, pistes et taille. Version par défaut choisie par résolution puis débit, jamais selon la connexion. Choix valable pour la session seulement. Progression attachée à l'œuvre, pas à la version | Le mainteneur aura plusieurs copies d'un même film pour ses tests et doit pouvoir passer de l'une à l'autre. |
| Identité visuelle | Rouge `#c81e1e` en **remplissage** (bouton de lecture, barre de défilement, éléments actifs) avec texte blanc dessus, même valeur en thème clair et sombre. Variante éclaircie réservée aux liens et petits textes colorés sur fond sombre. Couleur de thème du script d'installation | Mesuré : blanc sur ce rouge donne 5,7 pour 1 ; le même rouge en texte fin sur fond sombre ne donne que 3,2. |
| Noms de fichiers du mainteneur | Jamais dans le dépôt : ni code, ni tests, ni documentation, ni message de commit. Les tests utilisent des titres inventés couvrant les mêmes formes | Demande explicite du mainteneur. |
| Analyse des noms | Découpage sur la dernière année plausible, tout ce qui suit est écarté du titre, soulignement traité comme séparateur, casse et accents normalisés, étiquettes exploitées mais jamais opposables à l'analyse du fichier | Les fichiers sont posés à plat, sans dossier par film : le nom est la seule source. |
| Choix des pistes | Version, piste audio et piste de sous-titres se choisissent **sur la fiche**, avant le lancement. La décision de lecture n'est calculée qu'au moment du lancement | Évite de démarrer un transcodage pour rien avant de changer d'avis. |
| Image de titre | Type d'image distinct de l'affiche et du fond, récupéré et stocké comme tel | Le bandeau de la fiche affiche le titre sous forme d'image. |
| Champs techniques | Analyse du fichier stockée au niveau de détail attendu par la fiche, couleurs comprises (primaires, espace, courbe de transfert, profondeur) | Ces champs commandent la décision de conversion en SDR, ils ne sont pas décoratifs. |
| Chemin et taille du fichier | Visibles par l'administrateur seulement, contrairement à Emby qui les montre à tous | Ce sont des informations sur le serveur, pas sur le film. |
| État sur les cartes | Pastille de vu en coin de carte, dans les rangées comme dans les grilles | Relevé sur la référence, utile partout. |
| Textes qui peuvent déborder | Aucune hauteur fixe : le bloc occupe la place disponible, le texte la remplit, et le lien de dépliage n'apparaît que si le texte déborde vraiment | Emby coupe à hauteur fixe et laisse un vide quand le texte est court. Défaut à ne pas reproduire. |
| Emploi de l'accentuation | Nettement plus présente que chez Emby, mais réservée à ce qui est actif, sélectionné, en progression ou actionnable en premier. Neutre pour le texte courant, les grandes surfaces et les bordures ordinaires. Chaque emploi est un rôle nommé dans les jetons | Emby sous-emploie sa couleur ; étalée partout, elle ne signalerait plus rien. |
| Interface mobile | L'interface web doit être utilisable depuis le navigateur d'un téléphone | Presque gratuit si prévu au premier écran, coûteux ensuite. Le préchargement au survol n'existe pas au doigt : préchargement à l'apparition de la vignette. |
| Clavier | Vrais boutons et vrais liens, ordre de tabulation, contour visible, **et gestionnaire de focus directionnel écrit en même temps que la première grille** | Greffé après coup sur des grilles existantes, il coûte plusieurs fois plus cher. Prépare le client télévision. |
| Installation | Script shell unique reprenant les conventions du dépôt `Proxmox-Tools` du mainteneur : bilingue intégré, couleurs 256 niveaux désactivables, bannière, indicateur animé, encadrés, menu numéroté, sauvegarde avant modification, aucune dépendance | C'est la signature du mainteneur sur tous ses dépôts. |
| Publication | Une étiquette de version GitHub par version ; le serveur signale l'existence d'une version plus récente, sans jamais se mettre à jour seul ; vérification désactivable | L'administrateur décide du moment de la mise à jour. |
| Licence | GNU AGPL v3, comme les autres dépôts du mainteneur | Cohérence avec `Proxmox-Tools`. |
| Utilisateurs | Un utilisateur par défaut et un jeton de session dès la V0.1 | Toute donnée de progression est rattachée à un utilisateur dès le départ. |
| Réactivité | Rien de lourd sur le chemin de lecture : tout est précalculé à l'écriture | Exigence forte, voir le document 02. |
| Compilation | **Directement dans le LXC de production**, via une commande unique de mise à jour (récupérer, compiler en priorité basse, migrer, redémarrer) | Les PC du mainteneur sont sous Windows ; aucun transfert de fichier. |
| Ressources du LXC | 100 Go de disque, 16 Go de mémoire, 12 threads (monter à 16 si les autres services de l'hôte le permettent) | Le 16 Go et les threads servent surtout à la compilation ; le serveur lui-même est léger. |
| Matériel de transcodage | Intel Arc A380 passée au LXC via `/dev/dri`, distribution FFmpeg de Jellyfin recommandée | Décodage, encodage H.264, HEVC, AV1 et tonemapping HDR sur la carte. |
| Port | 2100, accès local en `ip:2100` | Libre de tout usage courant. Reverse proxy nginx (autre LXC) et HTTPS plus tard. |
| Configuration et répertoires | TOML dans `/etc/melyxar/melyxar.toml` ; données (base) dans `/var/lib/melyxar` ; cache d'images et segments de transcodage dans `/var/cache/melyxar` ; utilisateur système `melyxar` ; service `melyxar.service` | Conventions Debian, tout sur le NVMe du LXC, jamais dans les dossiers médias. |
| Métadonnées | TMDb avec une clé d'API propre au mainteneur, dans la configuration, jamais dans le dépôt | TMDb exige une clé ; Jellyfin et Emby en embarquent une dans leur code. Une clé par défaut embarquée pourra être ajoutée plus tard pour la distribution. |
| Journaux | Noms de médias censurés (quatre premiers caractères puis points de suspension, racine seule pour les chemins), réglage explicite pour révéler, désactivé par défaut | Les journaux sont partagés pour le débogage. |
| Vérification | Pas d'intégration continue GitHub. Compilation, `clippy` et tests dans l'environnement de travail avant chaque commit, puis compilation réelle dans le LXC par le mainteneur | Le mainteneur rapporte les erreurs directement. |

## Environnement de production

- Hôte Proxmox : Ryzen 7 3700X (8 cœurs, 16 threads), 32 Go DDR4, Intel Arc A380, NVMe. Noyau hôte `7.0.14-12-pve`.
- LXC Melyxar : Debian 13, **non privilégié**, 12 threads, 16 Go, 100 Go sur NVMe.
- Médias : quatre disques montés dans le LXC sous `/mnt/`, un dossier par disque, chacun contenant des dossiers `Films`, `Séries`, `Animés`, `Émissions`. Une bibliothèque regroupe donc les quatre dossiers de même nom. Les dossiers sont lisibles par tous les utilisateurs (droits `rwxrwxr-x`), la lecture ne demande donc aucun alignement d'identifiant. Une écriture future (fichiers annexes, suppression) demanderait que l'utilisateur `melyxar` appartienne au groupe propriétaire des dossiers. Les chemins réels vivent dans la configuration du serveur, jamais dans le dépôt.
- Films posés à plat dans leur dossier, sans sous-dossier par film.
- Bibliothèque de test : le dossier `Films` existant, environ 50 films.
- Première cible client : Brave / Chromium sous Windows, en réseau local.
- Le mainteneur ne lit pas le code : les journaux, la commande de diagnostic et la page de diagnostic sont conçus pour lui.

## Règles de travail

- **Langue du code : anglais.** Noms de crates, modules, types, fonctions, variables, tables et colonnes, routes de l'API, messages de commit, commentaires dans le code, journaux techniques : tout suit les standards anglais habituels.
- **Langue de tout le reste : français.** Documentation, explications, revues, décisions, échanges avec le mainteneur.
- Le em dash est banni de tout texte du projet.
- Textes d'interface toujours passés par l'internationalisation, anglais d'abord, français ensuite.
- Tout journal de débogage est protégé par un contrôle de type `BuildConfig.DEBUG` ou équivalent Rust (`cfg(debug_assertions)`).
- Pas de code mort, pas de contournement temporaire, pas de commentaire inutile.
