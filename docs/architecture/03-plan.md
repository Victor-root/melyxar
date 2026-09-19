# Plan par jalons

Chaque jalon est compilable et testable seul dans le LXC, avec un résultat visible pour le mainteneur. On ne passe au suivant que lorsque le précédent fonctionne en conditions réelles. L'état est tenu à jour ici.

Les fonctionnalités voulues et leurs conséquences sont détaillées dans [04-fonctionnalites.md](04-fonctionnalites.md).

Légende : à faire, en cours, terminé.

## Jalon 0 : socle

État : terminé.

Vérifié dans l'environnement de travail : la compilation passe, `cargo clippy` ne signale rien, la suite de tests est verte, `melyxar doctor` affiche FFmpeg, les accélérations matérielles, l'absence de `/dev/dri` et l'état d'accès de chaque racine, le service répond sur `/api/v1/system/info` et s'arrête proprement sur SIGTERM. Le script d'installation a été passé au vérificateur `shellcheck` puis exécuté en entier : vérifications système, paquets, compte et répertoires, récupération du dépôt, compilation en priorité basse, installation du binaire et écriture de la configuration se déroulent sans intervention. Seule la pose du service systemd n'a pas pu être exercée ici, faute de systemd dans l'environnement de travail ; c'est au mainteneur de la confirmer dans le LXC.

- Workspace Cargo avec les crates `core`, `config`, `database`, `ffmpeg`, `app`, `server`, `melyxar` (les autres arrivent quand elles servent).
- Configuration TOML (`/etc/melyxar/melyxar.toml`) : chemins des données, du cache, des transcodages, du binaire FFmpeg, port 2100, mode d'accès (HTTP ou HTTPS) prévu dès maintenant, bibliothèques déclarées.
- Base SQLite ouverte en WAL, migrations appliquées au démarrage. Première migration avec :
  - utilisateur, préférences utilisateur, jeton de session, **droits par utilisateur** (accès par bibliothèque, limite d'âge, téléchargement, suppression, sessions simultanées) ;
  - paramètres du serveur (nom, logo, fonds, CSS global, mode maintenance) ;
  - bibliothèque avec son **type explicite** (films, séries, animés, musique) et **plusieurs dossiers racines** par bibliothèque ;
  - tronc commun œuvre, source média, piste, **séparé des métadonnées spécifiques au domaine** ;
  - colonnes de **sonie EBU R128** sur les pistes audio, renseignées plus tard ;
  - vidéos annexes (bandes annonces), favoris, état de progression explicite, compteurs agrégés ;
  - **personnes et participations** (acteur, réalisateur, scénariste, personnage, ordre) ;
  - **collections** avec origine automatique ou manuelle, **étiquettes**, **listes de lecture** ;
  - **chapitres** et **segments repérés** (récapitulatif, générique de début, générique de fin) ;
  - **journal d'activité**, indexé par date et purgeable ;
  - **appareils connectés** avec leur jeton et leur dernière activité.
- Répertoire des fichiers envoyés par l'administrateur, dans les données et non dans le cache.
- Journalisation structurée avec temps par requête.
- **État d'accès de chaque racine** établi par un test réel : introuvable, illisible, lecture seule, lecture et écriture. Calculé au démarrage et à la demande, jamais sur le chemin de lecture.
- Commande `melyxar doctor` : version de FFmpeg et accélérations, accès à `/dev/dri`, état d'accès de chaque racine, mode WAL, tailles.
- **Script shell d'installation et de mise à jour**, aux conventions du dépôt `Proxmox-Tools` du mainteneur (bilingue intégré, couleurs 256 niveaux désactivables, bannière, indicateur animé, encadrés, menu numéroté, sauvegarde avant modification, aucune dépendance) : vérification du système, outils, utilisateur système et répertoires, récupération et compilation en priorité basse, configuration, service, migrations, redémarrage, adresse à ouvrir. Sert aussi à la mise à jour et à la désinstallation.
- Unité systemd. Entrées de menu pour la mise à jour, la sauvegarde, la restauration (arrêt, remplacement, vérification, redémarrage) et la désinstallation.
- Tests en place dès le départ : unitaires sur la logique pure, intégration sur base temporaire.
- Résultat visible : le service démarre, `ip:2100/api/v1/system/info` répond, `melyxar doctor` affiche un diagnostic lisible.

## Jalon 1 : scan et analyse

État : terminé, sauf l'identification qui appartient au jalon 2.

Vérifié dans l'environnement de travail : un premier scan enregistre et analyse de vrais fichiers, un deuxième ne change rien, un film retiré est marqué absent puis retrouve son identifiant quand il revient, une racine injoignable est ignorée sans vider les autres, une bande annonce est rattachée à son film, les sous-titres posés à côté deviennent des pistes, et aucun nom de fichier n'apparaît dans les journaux. La commande `melyxar scan` fait tout cela depuis un terminal et affiche le compte rendu.

- Crates `library`, `media_probe`, `jobs`.
- Scan incrémental d'une bibliothèque de films : ajouts, mises à jour, marquage absent, protection contre une racine injoignable.
- Analyse ffprobe de chaque fichier. Champs stockés par piste, correspondant à ce que la fiche doit afficher : pour la vidéo, codec, profil, niveau, résolution, ratio, entrelacement, images par seconde, débit, plage dynamique, couleurs primaires, espace colorimétrique, courbe de transfert, profondeur des échantillons, format des pixels, images de référence ; pour l'audio, langue, codec, disposition et nombre de canaux, taux d'échantillonnage, profondeur, débit, piste par défaut ; pour les sous-titres, langue, codec, par défaut, forcée, malentendants, interne ou externe. Les champs de couleur commandent la décision de conversion en SDR, ce ne sont pas des informations d'affichage.
- Détection des bandes annonces locales à côté des films.
- Analyse des noms de fichiers (titre, année) : fichiers posés à plat sans dossier par film, découpage sur la dernière année plausible, tout ce qui suit écarté du titre, soulignement traité comme séparateur, casse et accents normalisés. Jeu de tests écrit avec des titres inventés couvrant chaque forme, jamais des noms réels.
- Tâches de fond avec parallélisme borné, priorité basse, annulation. **L'identification est une étape séparée du scan**, rejouable seule sur un sous-ensemble.
- Lecture des fichiers d'accompagnement existants, en option et désactivée par défaut. Par défaut, rien n'est écrit dans les dossiers de médias ; l'écriture est une option arrivant au jalon 8 avec les droits.
- Résultat visible : après un scan de la bibliothèque de test (environ 50 films), la base contient les œuvres, sources et pistes ; le journal montre le déroulement sans nom complet de fichier.

## Jalon 2 : métadonnées et images

État : en cours. Le serveur, les règles et le stockage sont là ; ce qui reste demande l'interface du jalon 3.

Fait : le fournisseur TMDb derrière un trait, avec un fournisseur de remplacement pour les tests ; l'identification en tâche de fond, séparée du scan et rejouable seule ; le choix du bon film (titre exact, puis année, l'ordre du fournisseur en dernier) ; les identifiants déjà connus qui court-circuitent la recherche ; les textes par langue, genres, studios, personnes, participations, collections et bandes annonces distantes ; les affiches et fonds téléchargés une fois, convertis en WebP aux tailles fixes, servis sous un nom tiré de leur contenu, avec la couleur dominante de la carte ; les champs verrouillés à la main et la provenance de chaque champ.

Fait aussi : les photos des personnes, préparées en deux tailles pour les dix-huit noms que la fiche montre, jamais pour l'équipe technique, et jamais deux fois pour quelqu'un qui joue dans plusieurs films.

Fait depuis : l'identification à la main depuis la fiche de n'importe quel film, la raison de l'échec conservée et rendue au rapport avec le nom de fichier dont le titre a été lu, le regroupement des copies d'un même film et la séparation d'une copie mal rangée, et la lecture des noms fiabilisée sur une collection réelle de trois disques (326 films nommés sur 327, le dernier étant un épisode spécial de série que le fournisseur ne range pas du côté des films).

Fait depuis aussi : les images de titre, demandées dans la même requête que le reste, choisies dans la langue de la médiathèque puis en anglais, et préparées en deux largeurs comme les visages. Le lecteur les affiche à droite de la flèche de retour, à la place du titre écrit.

Reste : les films similaires, et la saisie directe d'un identifiant.

Vérifié dans l'environnement de travail : une affiche réelle est préparée en trois tailles avec sa couleur, une affiche inchangée n'est pas retéléchargée, un film sans affiche reste identifié, une clé refusée arrête la série en désignant la clé, et un fournisseur injoignable laisse les films en attente sans rien inventer. Sur cinq vrais films, les visages arrivent pour toute la distribution affichée et pour personne d'autre.

- Crate `metadata` avec le fournisseur TMDb derrière un trait.
- Identification des films, identifiants externes, provenance des champs, liens de bandes annonces.
- Personnes (acteurs, réalisateurs) avec photos, collections officielles, films similaires, classification d'âge.
- Textes stockés par langue, français prioritaire et repli anglais.
- Correction manuelle de l'identification depuis l'interface : recherche par titre et année, saisie directe d'un identifiant, choix parmi les propositions illustrées.
- Fichiers sans correspondance visibles dans la bibliothèque avec le repère « à identifier », plus une liste dédiée côté administration. Fournisseur injoignable : le scan continue, réessais espacés.
- Téléchargement des affiches, fonds et images de titre, génération des tailles fixes en WebP, couleur dominante, URL avec empreinte.
- Résultat visible : les œuvres ont titre, année, synopsis et affiches dans la base et le cache, et un film mal identifié se corrige en quelques clics.

## Jalon 3 : API et interface minimale

État : en cours. On peut parcourir la bibliothèque dans un navigateur, ouvrir une fiche et chercher un titre ; ce qui reste demande un compte connecté ou du travail de fond qui n'existe pas encore.

Fait : les routes de navigation (cartes paginées par curseur, fiche, images, filtres), le tri par titre, date d'ajout, année, note et durée, les filtres par genre, décennie et « à identifier », la recherche, la page d'accueil avec les derniers ajouts, la fiche complète avec sélecteur de version, distribution en visages, équipe, saga et bandes annonces, la page des tâches, les deux langues, les deux thèmes, et l'interface embarquée dans le binaire.

Reste : la liste « à voir plus tard » et les favoris (il faut d'abord un compte connecté), les films similaires, la correction manuelle depuis l'interface, la spécification OpenAPI et le canal temps réel.

Vérifié dans l'environnement de travail, sur de vrais films : la grille et la fiche s'affichent sans une seule erreur de console, le défilement continu passe de soixante à cent quarante-cinq cartes sans doublon puis annonce la fin, les flèches du clavier se déplacent d'une carte et d'une rangée, le contour de sélection est visible, et rien ne déborde de l'écran à quatre cents pixels de large.

- Routes de navigation : liste de cartes paginée par curseur, fiche, images, recherche globale sur titres, personnes et collections avec résultats groupés.
- Tri par titre, date d'ajout, année, note, durée. Filtres par genre, décennie, non vu, favoris, résolution, présence de sous-titres.
- Liste « à voir plus tard », distincte des favoris.
- Vrais boutons et vrais liens, ordre de tabulation respecté, contour visible sur l'élément sélectionné, et gestionnaire de focus directionnel écrit en même temps que la première grille.
- Interface utilisable depuis le navigateur d'un téléphone : grilles qui se réorganisent, zones tactiles suffisantes, aucune action accessible seulement au survol.
- Sélecteur de version sur la fiche quand plusieurs copies existent, décrites par résolution, codec, pistes et taille.
- Section « Récemment ajoutés » sur l'accueil, alimentée par la date d'ajout indexée.
- Couleur d'accentuation par défaut `#c81e1e`, avec sa palette dérivée et une variante éclaircie pour le thème sombre.
- Route publique d'identité visuelle (nom et logo), sans authentification et sans divulgation.
- Erreurs renvoyées sous forme de code et de données, jamais de phrase toute faite.
- Spécification OpenAPI générée, client TypeScript généré.
- **Flux temps réel** : un canal permanent par client connecté, transportant des événements typés et légers. Posé maintenant, utilisé ensuite par les tâches, les sessions et la maintenance.
- Interface React : grille virtualisée, fiche, cache des réponses, images adaptées. Disposition inspirée d'Emby pour l'accueil et la fiche.
- **Jetons de thème et internationalisation dès le premier composant** : aucune couleur ni chaîne en dur.
- Page d'accueil : bannière en haut, reprendre la lecture, récemment ajouté par bibliothèque, suggestions. Sections réordonnables et masquables par l'utilisateur.
- Fiche complète, sur la disposition relevée chez Emby : fond assombri, affiche, titre suivi des icônes de correction, ligne compacte (notes, année, durée, classification, genre, heure de fin estimée), ligne technique avec sélecteurs de version, de piste audio et de sous-titres **avant le lancement**, rangée d'actions (lecture, bande annonce, vu, favori, suppression, menu), accroche, synopsis tronqué avec dépliage, réalisateur.
- Rangées horizontales : distribution cliquable menant à la filmographie, chapitres, similaires avec pastille de vu sur les cartes.
- Section « À propos » : dernière lecture, genres, studios, liens externes, informations du média, puis une carte par piste vidéo, audio et sous-titres. **Chemin du fichier et taille réservés à l'administrateur**, contrairement à Emby.
- Section dédiée aux sagas : liste des coffrets, chacun ouvrable sur ses films.
- Lu et non lu, marquage manuel, favoris, étiquettes, listes de lecture.
- Modification manuelle d'une fiche avec verrouillage des champs.
- Résultat visible : la bibliothèque s'affiche dans Brave, navigation instantanée sur les 50 films.

## Jalon 4 : lecture Direct Play

État : en cours. Un film que le navigateur sait ouvrir se lit, et on revient là où on s'était arrêté.

Fait : la décision pure avec ses raisons, le profil demandé au navigateur lui-même (chaque combinaison essayée, jamais devinée d'après le nom du navigateur), le fichier servi par morceaux, le lecteur, la position enregistrée pendant la lecture et une dernière fois en partant, la reprise et le bouton distinct pour repartir du début, le choix des pistes pendant la lecture avec mémoire du choix (par film jusqu'à la piste, par langue pour les films suivants), le repliement stéréo comme entrée de la décision, la vitesse de lecture, l'image dans un coin et les raccourcis clavier.

Fait aussi : l'écran de réglages qui permet enfin de choisir tout cela. Le repliement, le niveau qui le suit, et les langues audio et sous-titres préférées vivaient dans la base avec leur valeur par défaut, sans écran ni route pour les changer. Les langues proposées sont celles que la bibliothèque contient vraiment, lues dans les pistes elles-mêmes.

Reste : la courbe de volume, à vérifier à l'oreille par le mainteneur. Un film que le navigateur ne sait pas ouvrir tel quel le dit clairement : le convertir est le jalon 5.

À savoir : les commandes de lecture elles-mêmes (barre de progression, volume, plein écran) sont aujourd'hui celles du navigateur. Tout ce qui les entoure est à nous : le choix des pistes pendant la lecture, la vitesse, l'image dans un coin, les raccourcis clavier, les étapes de préparation et l'apparence des sous-titres. C'est volontaire à ce stade, et ce n'est pas l'état final : un lecteur à nous est prévu au jalon 8.

À savoir : par défaut le serveur replie lui-même un son multicanal en stéréo plutôt que de laisser le navigateur le faire, parce que le repliement des navigateurs enterre les dialogues sous les effets. Cela coûte une reconstruction du son, la page le dit, et le réglage permet de laisser faire le navigateur.

Vérifié dans un vrai navigateur : le film se lit, la page explique pourquoi il se lit de cette façon, quitter à douze secondes et revenir repart à douze secondes, repartir du début repart bien de zéro, la piste audio choisie revient à la visite suivante, et un rapport de position qui arrive en retard est refusé.

Vérifié de bout en bout, de l'écran jusqu'au son : sur un film 5.1 que le navigateur sait lire tel quel, choisir « laisser faire l'outil » donne une lecture directe, choisir « dialogues, le soir » la transforme en reconstruction du son avec la raison affichée, et la commande réellement envoyée à FFmpeg porte la matrice de ce repliement, le gain choisi, puis la limitation, dans cet ordre.

- Crate `playback` avec la décision pure et ses raisons.
- Profil de capacités construit par le client web et envoyé au serveur.
- Fichier servi avec requêtes Range, balise vidéo native.
- Progression enregistrée en temps réel, copie locale côté client, position horodatée jamais écrasée par une plus ancienne.
- Reprise directe avec bouton distinct pour repartir du début.
- Courbe de volume vérifiée à l'oreille sur toute la course.
- Mémoire des langues audio et sous-titres, par utilisateur puis par série.
- Préférences de repliement stéréo (méthode et gain) prises en compte **comme entrée de la décision** : une méthode demandée force le transcodage audio, et la raison le dit.
- Vitesse de lecture, image dans l'image, raccourcis clavier.
- Résultat visible : un MP4 H.264 AAC se lit dans Brave, le journal explique pourquoi Direct Play a été choisi, fermer l'onglet en pleine lecture ne perd pas la position.

## Jalon 5 : remux et transcodage HLS

État : en cours. Un film que le navigateur ne sait pas ouvrir se lit quand même, le serveur le prépare au fur et à mesure, et rien ne lui survit.

Fait : la crate `streaming` avec ses sessions et sa playlist écrite par le serveur, le remux et le transcodage audio en HLS fMP4, le saut n'importe où dans le film, la limite de sessions simultanées, le balayage des sessions que plus personne ne regarde, le ménage au démarrage des dossiers laissés par un serveur qui s'est arrêté brutalement, la fermeture propre sur SIGTERM, les routes HTTP, le lecteur web qui va chercher les segments, les sous-titres texte convertis en WebVTT avec taille, couleur, contour, fond et hauteur réglables, la lecture des bandes annonces posées à côté du film, les étapes nommées de préparation avec le compte réel des segments prêts, et le repliement stéréo choisi par le spectateur réellement appliqué.

Corrigé depuis, mesuré sur un vrai film : après un saut, un segment portait une image d'un moment et un son d'un autre, jusqu'à dix secondes d'écart. L'image recopiée reculait jusqu'à son image clé pendant que le son, reconstruit, était rogné jusqu'à la position demandée. Seul le cas le plus courant était touché, une image que tous les navigateurs lisent et un son qu'aucun ne lit, et seulement après un saut, ce qui le faisait passer pour un défaut des fichiers.

Reste : où un saut atterrit quand le flux est simplement recopié. Mesuré : sur un film dont les images clés sont espacées de dix secondes, un saut atterrit six secondes trop tôt, parce qu'un flux recopié doit démarrer sur une de ses propres images clés. Le film se lit normalement à partir de là, mais ce n'est pas là qu'on a lâché le curseur. Un film entièrement reconstruit atterrit juste. La décision à prendre est notée dans le README des décisions.

Sur l'incrustation des sous-titres en images : le moteur la fait maintenant. FFmpeg accepte la commande produite et sort bien une vidéo, mais je n'ai pas pu prouver ici que les mots apparaissent vraiment à l'image : cet FFmpeg ne sait pas fabriquer un sous-titre en images à partir de rien (il ne convertit que image vers image), et aucun film de test n'en porte. À vérifier sur un vrai disque. Un sous-titre fait de mots, lui, n'est jamais peint sur l'image : l'outil peint des images sur des images, et nommer le film à l'intérieur d'une expression de filtre casse dès qu'un titre contient deux points ou une apostrophe. Les mots voyagent à côté, ce qui est de toute façon la seule forme qu'un spectateur peut couper.

À savoir : c'est le serveur qui écrit la playlist, pas FFmpeg. Il connaît la durée du film, donc il liste tous les segments avant qu'aucun n'existe. C'est ce qui rend le saut ordinaire : le lecteur demande le segment sur lequel il a atterri, et le serveur relance FFmpeg à cet endroit. L'astuce habituelle, laisser FFmpeg écrire la playlist au fil de l'eau, oblige à tout relancer et tout recharger dès que quelqu'un tire le curseur en avant.

À savoir aussi : les sous-titres voyagent à côté de l'image, jamais dedans. Choisir des sous-titres ne relance donc pas une conversion déjà en cours, et le lecteur ne les accroche à l'image qu'une fois celle-ci ouverte, parce que la mise en place du flux remet l'élément à zéro et effacerait ce qui y aurait été accroché avant.

Vérifié dans un vrai navigateur : un MKV que le navigateur refuse tel quel se lit, l'image avance, un saut à 80 % atterrit au bon endroit et continue de jouer sans erreur (le lecteur demande les segments 0 à 8, puis directement 18 à 22), les sous-titres d'un fichier .srt posé à côté du film s'affichent au bon moment et changent de taille et de hauteur en cours de lecture, une bande annonce locale se lit, fermer l'onglet ferme la session dans la seconde, une session que personne ne regarde plus est balayée au bout de deux minutes, et arrêter le serveur ne laisse ni FFmpeg ni dossier derrière lui.

Vérifié sur le serveur en vrai : un film HEVC que le navigateur ne sait pas décoder est reconstruit en H.264, les segments sortent à la bonne place dans le film, et les étapes de préparation se suivent réellement (trois puis cinq segments prêts sur les six attendus, puis prêt).

Fait depuis : une médiathèque se déclare depuis l'interface, en parcourant l'arborescence du serveur pour désigner ses dossiers. L'explorateur ne remonte que des dossiers, jamais un nom de fichier, et dit combien de vidéos chaque dossier contient directement, ce qui suffit à reconnaître le bon. Un dossier déjà surveillé par une autre médiathèque, ou qui en contient un, est refusé avec la raison. Une médiathèque et ses dossiers se renomment, et se retirent. Retirer **ne touche jamais au disque** : ce qui part est ce que le serveur sait, jamais un fichier. Le retrait balaie aussi tout ce que plus rien ne désigne derrière les films partis (personnes, collections de fournisseur, genres, studios, et leurs images dans le cache), en épargnant ce qu'un film encore présent utilise encore. L'écran annonce les nombres exacts avant de demander confirmation, dit en toutes lettres que rien n'est effacé du disque, et refuse tant qu'un travail tourne sur cette médiathèque.

Fait depuis : plus rien de ce qui touche au scan ne demande un terminal. L'heure de l'entretien, la forme des vignettes de la barre (intervalle, hauteur, découpage des planches) et la lecture des fichiers de description sont des réglages du serveur, gardés en base et modifiables depuis l'écran des réglages, avec l'avertissement que changer la forme remet tous les films devant l'entretien. Le fichier de configuration ne garde plus que ce qui dépend vraiment de la machine.

Fait depuis : le scan ne traverse plus les films lui-même. Les deux lectures intégrales, les points de départ et les vignettes de la barre, sont devenues **l'entretien** : deux travaux de fond qui tournent la nuit, au bouton, ou pendant le scan si la médiathèque le demande par un interrupteur de ses réglages, décoché par défaut. Le plafond de cinq mille fichiers par passe est parti avec elles : chaque lot redemande ce qui reste. Un scan et une identification se lancent en outre dans l'un de **trois modes**, comme chez Jellyfin : les fichiers nouveaux et modifiés, compléter ce qui manque, tout refaire. Chacun dit dans l'interface ce qu'il coûte.

Fait depuis : les miniatures de la barre de lecture. Une image toutes les dix secondes, rassemblées par cent dans des planches, produites en une seule lecture du film par un travail de fond, et la barre est désormais dessinée par Melyxar plutôt que par le navigateur, faute de quoi il n'y a nulle part où poser la miniature. Vérifié dans un vrai navigateur : l'image sous le curseur est bien celle du repère, aux deux bouts de la barre elle reste à l'écran, et la barre est là en plein écran. Les planches portent leur propre fiche sur le disque, pour qu'une table perdue ne coûte pas une deuxième nuit de lecture.

À vérifier par le mainteneur : le son EAC3 reconstruit en AAC. Le navigateur de test ici est un Chromium sans les codecs sous licence, il ne sait décoder ni AAC ni H.264 ; Brave le sait. Toute la chaîne est vérifiée jusqu'au décodage lui-même. La courbe de volume et le rendu des différents repliements stéréo se jugent à l'oreille, pas ici.

- Crate `streaming` : sessions, playlist générée par le serveur, segments à la demande, battement de cœur, nettoyage.
- Remux vers HLS fMP4, transcodage audio vers AAC, transcodage vidéo logiciel vers H.264, sous-titres texte vers WebVTT.
- Seeking par relance de FFmpeg à la position demandée.
- État de préparation de lecture exposé par étapes nommées, avec progression réelle sur la production du premier segment et la mise en tampon.
- Lecture des bandes annonces, locales et distantes.
- Apparence des sous-titres réglable : taille, couleur, contour, fond, position.
- Miniatures de la barre de lecture : planches produites au scan, barre dessinée par Melyxar, aperçu au survol.
- Repliement stéréo appliqué : méthodes en préréglages de filtre, gain de compensation suivi d'une limitation contre la saturation.
- Limite de sessions simultanées, arrêt propre sur SIGTERM, balayage au démarrage.
- Résultat visible : le premier jalon utile du document 01. Un MKV avec audio EAC3 se lit dans Brave, un seek à 80 % fonctionne, aucun FFmpeg ne survit à la fermeture de l'onglet.

## Jalon 6 : accélération matérielle

État : en cours.

Fait :

- VAAPI pour l'Intel Arc A380 : détection par **essai réel** au démarrage, un essai par codec, chaque refus conservé mot pour mot dans le journal et dans le rapport.
- Encodage H.264, HEVC et AV1 sur la carte, le meilleur codec que le client et la carte portent tous les deux.
- Conversion HDR vers SDR sur la carte (`tonemap_vaapi`), avec repli complet sur le logiciel si la carte ne sait pas la faire : une carte qui ne convertit pas rendrait un film gris, ce qui est pire qu'un film plus petit.
- Repli automatique sur le processeur quand la carte refuse un film en cours de session, une seule fois, avec la phrase de l'outil au journal.
- Choix de qualité par le spectateur (taille et débit), qui devient un plafond dans le profil client.
- Décodage sur la carte dans les codecs qu'elle a prouvé savoir lire, établis un par un, avec repli sur le processeur pour les autres.
- Un segment rendu dès que l'outil annonce l'avoir dépassé, au lieu d'attendre que le suivant soit produit par-dessus.
- Relevé des images clés, playlist découpée dessus quand l'image est recopiée : un saut atterrit où il vise au lieu de plusieurs secondes trop tôt. Travail de fond de l'entretien, repris là où il s'était arrêté, compté dans le rapport.
- Étape nommée sur chaque tâche, avec son compte à elle : on voit sur quelle passe le scan travaille et combien de films il a traités, au lieu d'une barre qui retombe à zéro sans explication.
- Reprise automatique après un redémarrage : une tâche coupée est marquée comme telle, et le serveur la relance seul en priorité basse. Une mise à jour en plein scan ne coûte plus rien.
- **Lecture directe de l'index du conteneur** pour relever les images clés, ce que fait Jellyfin et ce qui rend un saut instantané dans un lecteur de bureau. La crate `container` lit les tables d'un MP4, MOV ou M4V et les repères de recherche d'un MKV ou d'un WebM, sans lancer d'outil et sans traverser le film ; tout ce qu'elle ne sait pas lire retombe sur l'analyseur, inchangé. Mesuré sur un film de 477 Mo déjà en cache, donc au pire pour l'index : 1,8 ms en MKV et 0,7 ms en MP4 contre 431 ms, pour des listes identiques. Le journal dit à chaque entretien combien de films ont répondu de chaque façon.
- **Extraction des sous-titres pendant l'entretien** plutôt qu'à l'ouverture du film. Les sous-titres faits de mots sont entrelacés avec l'image d'un bout à l'autre, donc les sortir demande de faire défiler le fichier entier : mesuré à trois quarts de minute sur un film 4K, et cette attente tombait sur celui qui venait d'appuyer. C'est devenu la troisième tâche de l'entretien, à côté des images clés et des vignettes : elle tourne la nuit ou depuis le bouton, et les mots sont dans le cache avant que quiconque ouvre le film. Seuls les films portant une telle piste à l'intérieur entrent dans la file : un sous-titre en images est peint dans le film au moment où on le regarde, un sous-titre dans un fichier à côté est déjà le fichier qu'on en sortirait. Vider le cache des sous-titres remet ces films dans la file, les deux ne pouvant pas se désynchroniser. La lecture au moment de l'ouverture reste en filet de sécurité pour un film ajouté depuis le dernier entretien, et ne retraverse plus rien quand tout est déjà là.

Reste à faire :

- **NVENC pour Nvidia et VAAPI vérifié pour AMD.** Le mainteneur possède les deux. VAAPI couvre déjà AMD sur le papier, et rien ne l'a prouvé sur une vraie carte AMD ; Nvidia demande un chemin distinct. Les deux se branchent au même endroit : `HardwareAcceleration`, le nom de l'encodeur, et les filtres correspondants.
- **QSV pour Intel.** VAAPI marche sur l'Arc ; QSV est parfois plus rapide sur les cartes Intel récentes. À mesurer avant de l'ajouter, pas à supposer.
- Vignettes de chapitres : celles de la barre de lecture sont faites, en planches et converties en SDR quand la source est HDR ; **les vignettes de chapitre proprement dites restent à faire**. L'intervalle et la résolution se règlent dans le fichier de configuration, et l'écran qui les règlera est noté au jalon 8. La génération est activable par médiathèque depuis ses réglages.
- **Un fichier n'est traversé qu'une fois.** Direction voulue par le mainteneur, dans l'esprit de Jellyfin : prendre d'un fichier tout ce qu'il peut donner en une seule lecture, au lieu d'enchaîner des passes qui rouvrent chacune les mêmes films. **Largement atteint, et la dernière fusion a été essayée puis abandonnée sur mesure.** Les traversées ne sont plus des passes du scan mais des travaux de l'entretien, ce qui règle le coût pour celui qui lance un scan, et la lecture de l'index du conteneur a supprimé la traversée des images clés pour tous les formats courants. Restent celle des vignettes et celle des sous-titres.

  Fondre les deux, les mots voyageant dans la lecture des vignettes, a été codé et mesuré. **Deux résultats l'ont fait retirer.** D'abord le gain est minuscule : sur un film de 167 Mo, les vignettes coûtent 0,47 s, les mots seuls 0,09 s, et la lecture fusionnée 0,46 s. Sortir les mots ne coûte donc pas une traversée du fichier comme on le croyait en écrivant la règle, mais un sixième de la moins chère des deux passes : l'outil saute l'image au lieu de la décoder. Ensuite le prix est une panne de synchronisation : la lecture des vignettes ne décode que les images qui se suffisent (`-skip_frame nokey`, mesuré dix fois plus rapide, 0,47 s contre 5 s), et sous cette option l'outil décale les sous-titres qu'on lui demande en même temps du départ déclaré de leur piste. Vérifié : une piste déclarée à 1 s ressort une seconde trop tôt, alors que la lecture séparée la rend juste. Les deux contournements essayés sont pires : `-discard nokey` rend bien l'heure mais annule l'accélération (4,7 s au lieu de 0,47 s), et `-copyts` la rend aussi mais rend à l'outil les horodatages bruts, ce qui déplacerait la grille des vignettes sur tout film dont l'image ne commence pas à zéro, c'est-à-dire précisément les remux dont la médiathèque est pleine. Un sixième d'une passe ne vaut pas des sous-titres décalés.
- Résultat visible attendu : un film 4K HDR se lit en SDR avec moins d'un cœur de processeur utilisé, et ses vignettes sont en couleurs correctes.

## Jalon 7 : réactivité mesurée et tableau de bord

État : à faire.

- Générateur de bibliothèque synthétique (10 000, 50 000, 100 000 œuvres).
- Script de charge et comparaison aux budgets du document 02, avec et sans scan.
- Tableau de bord d'administration en temps réel : sessions de lecture et leurs décisions, charge processeur et mémoire, carte graphique, tâches de fond, file d'écriture, espace disque, dernières erreurs. Flux d'événements arrêté quand personne ne regarde.
- Détail d'une session en cours : utilisateur, appareil, œuvre, position, décision et raisons, débit, vitesse d'encodage et matériel utilisé.
- Journal d'activité consultable, avec purge automatique.
- Export de diagnostic en un clic.
- Résultat visible : les budgets sont tenus à 100 000 œuvres, chiffres à l'appui, et le tableau de bord montre l'activité en direct.

## Jalon 8 : personnalisation et administration

État : à faire.

Fait depuis, en avance sur ce jalon : la moitié serveur du refus de la conversion des couleurs, réglage général sous « Image » dans les paramètres, décoché par défaut. Demandé pour un processeur trop lent pour reconstruire l'image des films HDR d'une collection, plutôt que pour un écran réellement HDR. Le Dolby Vision sans couche de base compatible reste converti dans tous les cas, pour ne jamais laisser une image cassée à l'écran. L'interrupteur pendant la lecture et le réglage par spectateur, pensés pour l'écran plutôt que pour le processeur, restent à faire avec le reste de ce jalon.

- Écran d'administration : nom du serveur, logo, écran de démarrage, fond et apparence de la page de connexion.
- Couleur d'accentuation choisie par l'utilisateur, avec palette dérivée et vérification automatique du contraste.
- Modes clair, sombre et automatique, stockés dans les préférences.
- CSS personnalisé, au niveau du serveur pour l'administrateur et au niveau de chaque utilisateur, avec possibilité de le désactiver et adresse de secours sans CSS.
- Lecteur avec ses propres commandes à la place de celles du navigateur : barre de progression, volume, plein écran, titre du film et retour, le tout cohérent en fenêtre comme en plein écran, et rejoignant le choix des pistes, la vitesse et l'image dans un coin qui existent déjà autour. Les commandes du navigateur conviennent à une vidéo regardée vite fait : elles ne portent ni le nom de ce qu'on regarde, ni ce qui l'entoure, et elles changent d'un navigateur à l'autre. Prévu ici parce que c'est le jalon où l'interface cesse de ressembler à une installation par défaut.
- Mode maintenance : message, durée estimée, page dédiée avec le code d'état approprié, accès conservé pour l'administrateur, avertissement des lectures en cours, exemple de configuration nginx.
- Gestion des utilisateurs et de leurs droits : accès par bibliothèque, limite d'âge, téléchargement, suppression, sessions simultanées. Code à quatre chiffres pour les appareils de télévision déjà autorisés.
- Téléchargement d'un fichier, soumis au droit correspondant.
- Suppression d'une œuvre, avec case décochée par défaut pour effacer aussi le fichier du disque, réservée à l'administrateur, chemin résolu côté serveur et vérifié sous une racine déclarée, entrée au journal d'activité.
- Assistant de première configuration : langue, compte administrateur, mode d'accès. S'ouvre tant que la configuration initiale n'est pas terminée et saute ce que le script d'installation a déjà réglé. La partie médiathèques est faite et se réutilise telle quelle : l'explorateur de dossiers, le formulaire et le premier scan existent déjà dans les réglages.
- Écran de choix d'utilisateur avec avatars, désactivable.
- Liste des appareils connectés avec leur dernière activité et révocation individuelle.
- Page des bibliothèques affichant l'état d'accès de chaque racine. Les fonctions exigeant l'écriture (suppression sur disque, écriture des fichiers d'accompagnement) sont désactivées avec leur raison quand la racine est en lecture seule.
- **Refus de la conversion des couleurs, au choix du spectateur.** La règle automatique reste en place et reste le défaut : un film à large gamme est converti parce qu'aucun navigateur ici ne l'affiche correctement. Ce qui manque est de pouvoir dire non, à deux endroits. Un **interrupteur pendant la lecture**, valable pour cette lecture-là et qui la relance, puisque l'image est reconstruite autrement. Un **réglage dans les paramètres du spectateur**, « ne jamais convertir », qui devient son défaut et que l'interrupteur peut encore contredire film par film. Décoché par défaut dans les deux cas. Sur un écran réellement en HDR, convertir fait perdre ce que le film contient et coûte une reconstruction complète de l'image ; le serveur ne peut pas le deviner, seule la personne devant l'écran le sait. Deux choses à ne pas oublier en le faisant : une image Dolby Vision de profil 5 servie sans conversion est verte et violette et non délavée, donc l'interrupteur prévient à cet endroit ; et refuser la conversion ne donne pas la lecture directe pour autant, le film pouvant rester reconstruit pour son codec, son son ou ses sous-titres.
- **Longueur du saut des boutons reculer et avancer, réglable par l'utilisateur.** Dix secondes aujourd'hui, en dur. Deux valeurs distinctes, une par bouton, parce que reculer et avancer ne servent pas à la même chose : on recule pour réentendre une réplique, on avance pour passer un générique.
- Option d'écriture des fichiers d'accompagnement à côté des médias, désactivée par défaut, avec vérification préalable du droit d'écriture.
- Statistiques personnelles : temps de visionnage, films vus sur une période, genres préférés.
- Notification d'une version plus récente publiée sur GitHub, sans mise à jour automatique, vérification désactivable.
- Sauvegarde automatique quotidienne de la base et des fichiers envoyés, quelques copies conservées.
- Résultat visible : le serveur ne ressemble plus à l'installation par défaut, et une maintenance annoncée s'affiche proprement.

## Jalon 9 : accès et chiffrement

État : à faire.

- Quatre options présentées honnêtement : derrière un reverse proxy, certificat auto-signé, certificat fourni par l'administrateur, certificat reconnu obtenu automatiquement avec un nom de domaine.
- Service HTTPS par le serveur lui-même pour les options concernées.
- Résultat visible : l'administrateur choisit son mode dans l'interface et sait ce que chacun implique.

## Jalon 10 : séries

État : fait, correction des génériques à la main mise à part.

- Le rangement : un fichier d'une bibliothèque de séries devient un épisode d'une saison d'une série. Là où le chemin passe par un dossier de saison, c'est le dossier qui le contient qui nomme la série et tout ce qu'il contient y va ; partout ailleurs le nom du fichier est lu en premier et le dossier ne fait que combler ce qui manque. Les quatre rangements qu'on trouve vraiment sur un disque marchent : un dossier par série avec un dossier par saison, un dossier par série avec les épisodes à plat, tout en vrac dans un seul dossier, et des dossiers de saison nommés dans n'importe quelle forme ou absents.
- La lecture des noms : `S01E02` sous toutes ses ponctuations, `1x02`, la forme en toutes lettres dans les deux langues, l'épisode double, le numéro seul dont la saison vient du dossier. Deux formes sont refusées exprès : le nombre nu (`102`) et la forme croisée à un seul chiffre (`16x9`), qui sont aussi une année, une résolution et un format d'image.
- La navigation : une série rend ses saisons, une saison ses épisodes, et chacune porte le chemin de retour, le tout dans la seule requête dont la page est déjà faite.
- Les métadonnées : le fournisseur est interrogé sur son catalogue des séries, et une série nommée fait décrire ses saisons et leurs épisodes. Les règles qui choisissent la bonne réponse sont les mêmes que pour un film, écrites une seule fois.
- Le confort : la série dit par quel épisode reprendre, un épisode enchaîne sur le suivant tout seul, une saison dit combien d'épisodes restent à voir et un épisode vu porte une coche.
- Le saut de générique : lu sur les chapitres que le fichier nomme lui-même, avec un bouton qui dit ce qu'il saute. Et, pour les fichiers qui ne nomment aucun chapitre, c'est-à-dire presque tous, **repéré en écoutant les épisodes d'une saison et en leur demandant ce qu'ils ont en commun**. Quatrième tâche de l'entretien, à côté des images clés, des sous-titres et des vignettes : sa propre ligne, son propre compte en saisons, son propre bouton, son propre arrêt. Ce qui reste à faire est la correction à la main.

Corrigé sur le retour de la vraie collection : une série dont deux saisons venaient d'équipes qui écrivent son titre dans deux langues se coupait en deux séries, dont une que le fournisseur ne reconnaissait sous aucun des deux noms ; une saison comme un épisode se voyaient redemander à chaque passage une image de titre qu'aucun fournisseur ne dessine pour eux, ce qui remplissait le journal de milliers de lignes sans objet ; et une série rangée dans un dossier de saison, mais dont les fichiers ne portent qu'un numéro et un titre, laissait ses cinquante-deux épisodes chacun seul dans la grille, sans série ni saison. Puis, en ajoutant un second disque à la même bibliothèque : la même série écrite autrement sur les deux disques était réunie par le fournisseur, et cette mise en commun, écrite pour deux copies d'un même film, effaçait la série qui partait avec toutes ses saisons, tous ses épisodes et tous leurs fichiers ; et la ligne qui dit qu'un fournisseur n'a pas d'image était répétée une fois par épisode. Puis, en relisant le journal du passage suivant : une série renommée par le fournisseur n'était plus reconnue par le scan, qui l'écrivait une deuxième fois à chaque fichier ajouté dedans. Et, sur signalement du mainteneur : un film dont la piste sonore commence après l'image était servi avec un trou que le navigateur ne traversait pas, donc avec le son en avance du début à la fin.

Vérifié dans un vrai navigateur contre un serveur qui tourne : les quatre rangements en même temps, une vraie série identifiée avec ses affiches de saison et ses épisodes nommés, un fichier que personne n'a su numéroter qui reste visible, trois épisodes qui s'enchaînent tout seuls, et le bouton de saut qui apparaît dans le générique, disparaît en plein film et revient au générique de fin.

## Après la V0.1

- Séries : **corriger un générique à la main** depuis la fiche d'un épisode, ce qui passe devant ce que le fichier dit et devant ce que l'écoute a trouvé. La place est déjà faite dans le modèle, il manque l'écran.
- Séries : **ranger** à nouveau des épisodes déjà rangés quand les règles s'améliorent. Le **nom** d'une série encore sans nom est désormais relu à chaque scan, comme celui d'un film, et une série mal nommée se répare donc toute seule. Ce qui reste est le rangement lui-même : un épisode déjà accroché à une saison n'est jamais rattaché ailleurs, parce que son fichier n'a pas bougé, donc un épisode mal placé le reste. La seule façon d'en sortir est de retirer la médiathèque et de la remettre, ce qui ne touche à aucun fichier mais fait tout redemander au fournisseur.
- Séries : l'affiche d'une saison ou la vignette d'un épisode qui n'est pas arrivée le jour de l'identification n'est redemandée que le jour où la série l'est à nouveau. Un film, lui, a sa passe de rattrapage ; une saison ne peut pas l'avoir telle quelle, puisqu'elle ne se demande jamais toute seule mais par la série au-dessus d'elle.
- Animés : fournisseur de métadonnées adapté et numérotation propre au domaine, la numérotation absolue n'étant pas lue aujourd'hui.
- Plusieurs fournisseurs de métadonnées : un second catalogue interrogé quand le premier ne connaît pas un film, des règles claires pour départager deux réponses, et la provenance restant visible champ par champ. Constaté en conditions réelles : un téléfilm rattaché à une série existe chez le fournisseur actuel du côté des séries et pas du côté des films, donc aucune recherche de film ne le trouvera jamais, alors qu'un catalogue construit sur les données IMDb le classe comme film. Les autres serveurs y arrivent parce qu'ils ont un second catalogue sous la main, pas parce que leur lecture des noms est meilleure.
- Bibliothèque musicale : modèle artiste, album, morceau, fournisseur MusicBrainz, navigation dédiée, listes de lecture.
- Normalisation audio : c'est la vraie réponse au niveau sonore, et le gain de repliage appliqué aujourd'hui à toutes les pistes tient la place en attendant. Mesure de sonie au scan, application au gain à la lecture, modes morceau et album, compression de plage dynamique pour les films.
- Plusieurs utilisateurs avec écran de connexion complet et gestion des droits.
- Clients natifs : un projet Android, base commune (API, session, cache, lecteur), interface télévision d'abord, interface téléphone ensuite.
- Émissions : type de bibliothèque à part entière (documentaires et programmes de télévision), réutilisant le modèle série, saison, épisode.
- Import ponctuel de l'historique de visionnage et des favoris depuis une installation Jellyfin existante.
- Recherche et téléchargement de sous-titres en ligne, sur demande explicite.
- Contrôle à distance d'une session de lecture depuis un autre appareil, sans priorité.
- Interface ambitieuse, surveillance des dossiers en temps réel.
