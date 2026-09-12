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
| Accès base | SQLx, SQL uniquement dans la crate `database`. **Requêtes vérifiées par des tests d'intégration sur une base réelle plutôt que par les macros à la compilation** | Décision révisée au moment d'écrire le code. Les macros imposeraient de régénérer et commiter des métadonnées à chaque changement de requête, une étape de plus qui peut casser la compilation dans le LXC. Chaque requête est exécutée par un test contre une base migrée, ce qui attrape la même classe d'erreur sans cette fragilité. Règle qui en découle : **toute requête doit être couverte par un test**. |
| FFmpeg | Programme externe, jamais les bibliothèques `libav*`, chemin configurable, une seule crate `ffmpeg` lance des binaires | Robustesse, mise à jour indépendante, accélération matérielle simple. |
| Streaming | HLS en fragments fMP4, playlist générée par le serveur, segments à la demande | Seeking instantané, un seul pipeline pour remux et transcodage. |
| Client web | React, TypeScript, Vite, hls.js, client TypeScript généré depuis la spécification OpenAPI | Écosystème d'interface le plus riche ; le frontend ne connaît que l'API. |
| Structure | Monolithe modulaire, workspace Cargo, direction unique des dépendances | Frontières imposées par le compilateur. |
| Modèle de données | Œuvre, source média et piste sont trois entités distinctes ; identifiants internes UUID v7 ; identifiants externes dans une table à part ; chemins relatifs à une racine ; millisecondes et UTC partout | Remplacer un fichier ne doit jamais effacer l'historique. |
| Modèle multi-domaines | La bibliothèque a un type explicite (films, séries, musique) ; le tronc commun (source, piste, progression, favoris, images) est séparé des métadonnées propres à chaque domaine | La musique est prévue à terme ; un schéma qui suppose « un film » imposerait une refonte. |
| Sonie audio | Colonnes EBU R128 sur les pistes audio dès la première migration, mesurées au scan, appliquées au gain à la lecture | Sans la colonne, toute la bibliothèque devrait être réanalysée plus tard. |
| Repliement stéréo | Plusieurs méthodes proposées, celle du standard industriel par défaut, plus un gain de compensation suivi d'une limitation pour éviter la saturation. Préférence par utilisateur avec valeur par défaut du serveur. Méthodes définies comme préréglages de filtre dans la crate des commandes | Sans repliement soigné, dialogues inaudibles et effets assourdissants, le défaut le plus courant des serveurs multimédias. |
| Repliement et décision | **Une préférence de repliement force le transcodage audio** et devient donc une entrée du moteur de décision, exposée dans ses raisons. L'interface précise que le réglage ne s'applique pas en lecture directe | Sans cela, le réglage semble sans effet sur les fichiers lus directement, ce qui est pire qu'un réglage absent. |
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
| Temps réel | Un seul flux permanent par client connecté, transportant des événements typés et légers (bibliothèque modifiée, tâches, sessions, maintenance). Le client agit toujours par les routes ordinaires | Commencer par de l'interrogation en boucle imposerait de réécrire la couche de données du client. |
| Fichiers d'accompagnement | Par défaut tout sur le disque système, rien à côté des médias. **Écriture et lecture des fichiers d'accompagnement proposées en option**, désactivées par défaut. Quand l'écriture est activée : droit vérifié d'abord, écriture en tâche de fond, et la base reste la source de vérité | Le défaut permet des disques en lecture seule et une sauvegarde complète du seul serveur ; l'option sert à ceux qui veulent une bibliothèque autonome. |
| Droits sur les racines | Quatre états distingués (introuvable, illisible, lecture seule, lecture et écriture), établis par un **test réel d'accès** et non par lecture des droits affichés. Visibles dans l'assistant, l'administration, le tableau de bord et la commande de diagnostic. Une fonction exigeant l'écriture est désactivée avec sa raison plutôt que de tomber en panne | Jellyfin et Emby laissent dans le flou et le problème se découvre au pire moment. Les droits affichés mentent dès qu'il y a groupes, montages réseau ou conteneur non privilégié. |
| Port | 2100, vérifié dans le registre officiel : attribution historique sans usage réel. Seul conflit réaliste, le service FTP du module XML d'Oracle Database, hors de propos ici. Modifiable en configuration, et le script vérifie qu'il est libre à l'installation | Choisi sur vérification, pas de mémoire. |
| Fichiers non identifiés | Visibles dans la bibliothèque avec le repère « à identifier », plus une liste dédiée côté administration | Un fichier mis de côté invisible est un fichier oublié. |
| Fournisseur injoignable | Le scan ne s'arrête jamais : fichiers enregistrés et analysés, identification mise en attente, réessais espacés | Impose que l'identification soit une étape séparée du scan, rejouable seule. |
| Sessions | Un jeton par appareil, de longue durée, révocable individuellement, avec la liste des appareils et leur dernière activité | Pas de reconnexion permanente sur la télévision, et reprise de contrôle possible si un appareil est perdu. |
| Tests | Unitaires sur la logique pure, intégration sur base temporaire, chaîne de lecture sur fichiers vidéo générés à la volée | Aucun contenu réel nécessaire, et les cas difficiles se fabriquent à la demande. |
| Restauration | Entrée du menu du script d'installation : arrêt, remplacement de la base et des fichiers envoyés, vérification, redémarrage | La sauvegarde sans restauration testée ne vaut rien. |
| Bibliothèque « Émissions » | Type à part entière (documentaires et programmes de télévision), réutilisant le modèle série, saison, épisode et le même fournisseur | Navigation distincte des séries de fiction, sans dupliquer le modèle. |
| Licence | GNU AGPL v3, comme les autres dépôts du mainteneur | Cohérence avec `Proxmox-Tools`. |
| Utilisateurs | Un utilisateur par défaut et un jeton de session dès la V0.1 | Toute donnée de progression est rattachée à un utilisateur dès le départ. |
| Réactivité | Rien de lourd sur le chemin de lecture : tout est précalculé à l'écriture | Exigence forte, voir le document 02. |
| Compilation | **Directement dans le LXC de production**, via une commande unique de mise à jour (récupérer, compiler en priorité basse, migrer, redémarrer) | Les PC du mainteneur sont sous Windows ; aucun transfert de fichier. |
| Ressources du LXC | 100 Go de disque, 16 Go de mémoire, 12 threads (monter à 16 si les autres services de l'hôte le permettent) | Le 16 Go et les threads servent surtout à la compilation ; le serveur lui-même est léger. |
| Matériel de transcodage | Intel Arc A380 passée au LXC via `/dev/dri`, distribution FFmpeg de Jellyfin recommandée | Décodage, encodage H.264, HEVC, AV1 et tonemapping HDR sur la carte. |
| Accès | Local en `ip:2100` pour commencer, reverse proxy nginx (autre LXC) et HTTPS plus tard | Voir la ligne « Port » pour le choix du numéro. |
| Configuration et répertoires | TOML dans `/etc/melyxar/melyxar.toml` ; données (base) dans `/var/lib/melyxar` ; cache d'images et segments de transcodage dans `/var/cache/melyxar` ; utilisateur système `melyxar` ; service `melyxar.service` | Conventions Debian, tout sur le NVMe du LXC, jamais dans les dossiers médias. |
| Métadonnées | TMDb, avec la clé de l'application embarquée dans le binaire, brouillée plutôt qu'écrite en clair. Aucun réglage à faire pour l'utiliser | La clé identifie l'application et non la personne qui l'installe, comme chez Jellyfin et Emby : personne n'a de compte à créer. Le brouillage évite qu'elle traîne dans une recherche ou une capture d'écran, sans prétendre la rendre secrète. Elle n'ouvre qu'un catalogue de films public ; si elle est un jour désactivée, on en embarque une autre. |
| Journaux | Noms de médias censurés (quatre premiers caractères puis points de suspension, racine seule pour les chemins), réglage explicite pour révéler, désactivé par défaut | Les journaux sont partagés pour le débogage. |
| Œuvre et fichier | Une œuvre n'est jamais un fichier. Deux copies d'un même film sont deux fichiers d'une même œuvre, reconnus par le titre trié et l'année | C'est ce qui met un sélecteur de version sur la fiche au lieu du même titre deux fois dans la grille, et ce qui fait survivre l'historique au remplacement d'un fichier. |
| Fichier disparu | Marqué absent, jamais supprimé. Une racine illisible est ignorée sans toucher aux autres racines de la bibliothèque | Un disque débranché doit coûter une soirée, pas une bibliothèque. |
| Sous-titres externes | Un sous-titre dans son propre fichier est une piste de l'œuvre. Langue, forçage et malentendants sont lus dans le nom du fichier ; un mot qui pourrait être un groupe de release n'est jamais pris pour une langue | Une langue que personne ne parle dans le sélecteur est pire que pas de langue du tout. |
| Fichiers de description (`.nfo`) | Lecture facultative, **désactivée par défaut**. Seuls les identifiants des fournisseurs sont repris, jamais un titre ni un synopsis | Un identifiant peut être vérifié auprès du fournisseur ; un titre serait cru sur parole et écraserait silencieusement le sien. |
| Tâches de fond | Écrites en base avant de démarrer, annulables, avec progression. Deux tâches de même nature ne tournent jamais sur le même sujet à la fois ; un redémarrage clôt ce qu'il a interrompu | Un écran doit pouvoir montrer ce qui tourne, et l'arrêt doit arrêter le travail, pas seulement ignorer sa réponse. |
| Analyse des fichiers | Étape distincte du parcours des dossiers, reprise par le scan suivant, bornée à quelques fichiers à la fois | Un scan doit laisser intacte la lecture qui se déroule à côté. |
| Client HTTP sortant | `reqwest` avec `rustls`, jamais OpenSSL, délais d'attente explicites et un seul client réutilisé | `rustls` évite de dépendre d'une bibliothèque système et donc d'un paquet de développement de plus à installer dans le LXC. Un client partagé garde les connexions ouvertes vers le fournisseur au lieu d'en rouvrir une par film. |
| Fournisseur de métadonnées | Derrière un trait, avec un fournisseur de remplacement dans les tests. Un fournisseur injoignable n'arrête jamais un scan : la tâche est réessayée à intervalles qui s'écartent | Le scan et l'identification sont deux étapes séparées ; la bibliothèque reste consultable même si TMDb est en panne. |
| Images | Téléchargées une fois, converties par FFmpeg en WebP aux largeurs fixes, servies sous un nom tiré de leur contenu. Jamais redimensionnées à la demande | Un serveur qui redimensionne à chaque requête passe son après-midi sur la même affiche. Le nom permet au navigateur de garder une image pour toujours et de voir la nouvelle le jour où elle change. |
| Couleur de carte | Moyenne de l'affiche, lue en la réduisant à un pixel | Une carte a une couleur à montrer avant l'arrivée de son image. |
| Interface | React construite ici et **le résultat construit est commité**, puis embarqué dans le binaire Rust | Le LXC n'a alors aucun outil JavaScript à installer et la commande de mise à jour ne change pas : elle compile le binaire, et l'interface est dedans. Le jour où des binaires précompilés sont distribués, rien ne bouge. |
| Grille | Défilement continu par curseur, avec `content-visibility` plutôt qu'une bibliothèque de virtualisation | Le navigateur saute le dessin de ce qui est hors écran sans dépendance supplémentaire ni gestion manuelle des hauteurs. |
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
