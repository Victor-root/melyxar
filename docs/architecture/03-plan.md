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

Reste : les films similaires. La saisie directe d'un identifiant est faite, dans la fenêtre d'identification.

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

Fait : les routes de navigation (cartes paginées par curseur, fiche, images, filtres), le tri par titre, date d'ajout, année, note et durée, les filtres par genre, décennie et « à identifier », la recherche, la page d'accueil avec les derniers ajouts, la fiche complète avec sélecteur de version, distribution en visages, équipe, saga et bandes annonces, la page des tâches, les deux langues, les deux thèmes, et l'interface embarquée dans le binaire (servie depuis son propre dossier depuis, voir le README des décisions).

Fait depuis : la fiche refaite sur la disposition d'Emby avec l'identité de Melyxar. Le haut tient dans l'écran sous la barre (le synopsis s'élargit puis se coupe sur une ligne entière seulement s'il le faut), les pistes audio et sous-titres se choisissent avant la lecture et sont retenues comme dans le lecteur, la barre de reprise se remet à jour en quittant le lecteur, vu, favori et le menu des cartes en boutons ronds, la bande annonce avec son icône, puis les rangées de la distribution, des chapitres (vignettes tirées des planches de la barre de lecture, lecture au chapitre d'un clic) et « Basé sur le genre », et enfin les versions en panneaux. Chaque personne créditée a sa page : photo, naissance, âge, lieu, biographie demandée au fournisseur à la première ouverture, et ses films et séries présents sur le serveur.

Fait depuis aussi : l'onglet porte le nom de ce qui est en lecture, et Melyxar s'installe comme une application, sur ordinateur comme sur téléphone, avec une icône dessinée par le serveur dans la couleur du logo de la personne.

Reste : la liste « à voir plus tard » (sa table existe, rien ne s'en sert encore) et la spécification OpenAPI. Faits depuis : la correction manuelle depuis l'interface, et un canal temps réel pour l'administration (journal et lectures en cours), pas encore pour le reste de l'interface.

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

Le lecteur a désormais ses propres commandes (barre de progression, volume, plein écran, titre et retour), en plus du choix des pistes, de la vitesse, de l'image dans un coin, des raccourcis clavier, des étapes de préparation et de l'apparence des sous-titres.

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

État : fait. Le tableau de bord, le détail d'une lecture, le journal et l'export de diagnostic sont dans l'administration (voir le jalon 8). Seule la file d'écriture n'y apparaît pas.

Fait :

- **Générateur de bibliothèque synthétique.** `melyxar bench fill --works 100000` écrit une bibliothèque inventée à côté des vraies, dans la forme qu'une vraie a : titres répartis sur tout l'alphabet, soixante ans de sorties, genres et studios partagés comme un catalogue les partage, quelques milliers d'acteurs crédités sur l'ensemble, et un sixième des œuvres en séries avec leurs saisons et leurs épisodes. Cent mille œuvres en quarante secondes. Aucun fichier n'est écrit sur un disque : les affiches existent en lignes, parce que ce qu'un fichier coûte à servir ne change pas avec la taille de la collection. `melyxar bench empty` la retire et rend la place au disque.
- **Banc d'essai intégré**, `melyxar bench run`, plutôt qu'un outil de charge extérieur : il joue les pages qu'un spectateur ouvrirait contre le serveur en marche, deux cents fois chacune, et met ses centiles en face des budgets écrits. Sort en erreur quand un budget est dépassé. Entrée de menu dans le script d'installation, qui remplit, mesure et retire d'un coup.
- **Chronomètre par requête** : chaque réponse porte son temps dans l'en-tête `Server-Timing`, visible dans l'onglet Réseau du navigateur, et tout ce qui dépasse quelques millisecondes écrit une ligne de journal sous l'étiquette `timing`.
- **Budgets tenus à cent mille œuvres, chiffres à l'appui** : grille 5,5 ms (budget 30), fiche 3,3 ms (budget 20), recherche 5,5 ms (budget 30), menus 0,4 ms, accueil 3,2 ms. Deux d'entre eux ne l'étaient pas avant d'être mesurés, et ont été corrigés : les menus d'une médiathèque prenaient 690 ms et la page d'accueil 156 ms, toutes deux à compter la collection entière à chaque visite.

Reste à faire :

- Mesure pendant un scan et pendant deux transcodages. Rien à coder pour la première : il suffit de lancer un scan et de lancer le banc pendant qu'il tourne. Demande une vraie collection, puisqu'une bibliothèque inventée n'a aucun fichier à lire.
- Test automatisé de l'interface (Playwright) pour les temps côté client. Attend l'interface définitive.
- Tableau de bord d'administration en temps réel : sessions de lecture et leurs décisions, charge processeur et mémoire, carte graphique, tâches de fond, file d'écriture, espace disque, dernières erreurs. Flux d'événements arrêté quand personne ne regarde.
- Détail d'une session en cours : utilisateur, appareil, œuvre, position, décision et raisons, débit, vitesse d'encodage et matériel utilisé.
- Journal d'activité consultable, avec purge automatique.
- Export de diagnostic en un clic.
- Résultat visible : les budgets sont tenus à 100 000 œuvres, chiffres à l'appui, et le tableau de bord montre l'activité en direct.

## Jalon 8 : personnalisation et administration

État : en cours. Les comptes existent pour de vrai ; il reste l'écran d'administration et le reste de la personnalisation.

### Chantier de l'administration

Commencé : l'interface d'administration est refaite de zéro, dans son propre espace avec une barre latérale, et les réglages personnels sont séparés de ceux du serveur (voir le README des décisions). Il réunit le tableau de bord du jalon 7 et l'écran d'administration de celui-ci. Cinq lots, chacun essayé dans le LXC avant le suivant :

1. **Fait. Le cadre** : barre latérale et ses douze sections, « Mes réglages » séparés, les écrans qui existent déjà redessinés et rangés à leur place, et un premier Résumé avec ce que le serveur sait déjà dire. Ce qui n'est pas encore branché est dessiné, éteint et marqué « Bientôt ».
2. **Fait. Les mesures du système** : processeur, mémoire, disques, réseau, charge, température quand le conteneur la voit, carte graphique, avec leur historique en base et leurs courbes. Reste à voir dans le LXC ce que le conteneur laisse lire de la température et de la carte.
3. **Fait. Lecture et Transcodage** : les lectures en cours sur chaque appareil, lecture directe comprise, suivies toutes les deux secondes : qui, quoi, où en est le film, en pause ou non, la méthode et ses raisons, ce que contient le fichier et ce qui en est refait, par la carte graphique ou le processeur, et la vitesse de conversion. Le détail d'une lecture se déplie sous sa carte. Un administrateur peut arrêter une lecture : le lecteur quitte le film et dit pourquoi, et la conversion est fermée par le serveur si le lecteur n'obéit pas. Le panneau « Activité en cours » du Résumé compte les lectures, les transcodages et les lectures directes. L'historique des lectures viendra avec le journal d'activité.
4. **Fait. Le journal d'activité**, les alertes « À surveiller » et la cloche de la barre du haut. Chaque connexion, refus, déconnexion, changement de mot de passe, compte créé ou retiré, visionnage (avec le temps vraiment joué), tâche terminée ou en échec, suppression, démarrage et arrêt du serveur est une ligne, gardée le nombre de jours réglé dans les Paramètres. Le journal se lit par famille sur la page Journal, en historique des lectures sur la page Lecture et en dernières lignes sur le Résumé. « À surveiller » réunit les voyants en défaut, les connexions refusées et les tâches en échec du jour, les conversions qui ne suivent pas et les titres à identifier, chacun menant où il se règle ; la cloche les compte et permet de les marquer comme vus.
5. **Fait. Utilisateurs et Appareils.** Utilisateurs : la page liste chaque compte avec son rôle, ses médiathèques, ses appareils connectés et sa dernière activité, et permet de le créer, le renommer, lui donner ses droits (administrateur, médiathèques une par une, retirer et supprimer du disque, lectures simultanées), lui mettre un mot de passe, le déconnecter de partout et le supprimer. Le serveur n'est jamais laissé sans administrateur, un administrateur ne défait rien de son propre compte depuis cette page, et chaque changement est au journal. Avant d'ouvrir la page, un audit de chaque route a trouvé des chemins qui échappaient au filtre des médiathèques (fichier lu directement, sous-titres, vignettes, film de calibration, page suivante d'une grille, mémoire des pistes, signal de lecture) et les chemins de fichiers envoyés à tous : tous refermés, chacun avec son test. Appareils : la page liste chaque appareil connecté, tous comptes confondus, avec le navigateur et le système, le compte, la date de connexion, la dernière activité et si la session s'arrête à la fermeture du navigateur ; un appareil se déconnecte seul et ce qu'il lisait s'arrête. Chacun retrouve ses propres appareils dans son profil et peut les déconnecter un par un, ou tous sauf celui où il se trouve d'un seul bouton. Chaque navigateur n'a plus qu'une session par compte : se reconnecter depuis le même navigateur remplace la précédente. Chaque déconnexion d'un appareil est au journal. Restent, hors de ce lot : le code des télévisions, le droit de télécharger et la limite d'âge.

### Comptes, connexion et droits

Fait : la crate `auth` (mots de passe hachés par une fonction lente et gourmande en mémoire, jetons de session tirés au hasard) ; les sessions rangées dans la table des appareils, sans date de fin, vivantes tant qu'elles servent, jamais refusées pour leur âge et supprimées par le balayage quotidien au bout d'un an sans usage, avec leur dernier usage réécrit au plus une fois par heure ; un portier unique par lequel tout passe, fermé sauf pour quatre adresses ; le cookie fermé aux scripts, limité au site, exigeant le chiffrement seulement quand le serveur en sert ; les gestionnaires qui déclarent ce dont ils ont besoin, compte ou administrateur, le compilateur refusant qu'ils lisent un compte non demandé ; le journal, le diagnostic, l'explorateur de dossiers, les travaux et la gestion des médiathèques passés en administrateur ; le frein après dix mots de passe erronés d'affilée ; la page de connexion et l'assistant de premier démarrage, où la personne choisit son nom ; les médiathèques autorisées appliquées partout, avec la règle dite à voix haute plutôt que lue dans une absence de lignes ; la langue, le thème et la couleur d'accentuation rattachés au compte ; les commandes de terminal pour lister, créer, limiter, retirer un compte et y remettre un mot de passe, plus l'entrée de menu qui va avec.

Fait depuis : l'écran de gestion des comptes dans l'administration ; la limite de lectures simultanées, comptée sur les appareils en train de lire ; la liste des appareils connectés avec déconnexion d'un seul, dans l'administration et dans le profil de chacun ; une seule session par navigateur.

Reste : le code à quatre chiffres pour les télévisions déjà autorisées ; et la limite d'âge, reportée exprès (voir le README des décisions).

Vérifié dans un vrai navigateur contre un serveur qui tourne : la porte sur un serveur neuf et sur un serveur configuré, le mauvais mot de passe refusé, les deux mots de passe attrapés quand ils diffèrent, la bibliothèque rendue avec le bon nom dans la barre, une machine réglée en thème clair qui adopte le thème sombre, le français et une couleur de son compte. Et contre le serveur directement : une adresse fermée refusée sans cookie, un jeton inventé qui ne nomme personne, les images et les segments de film fermés eux aussi, la porte du premier compte qui se referme, dix mauvais mots de passe répondus avec le temps à attendre. Trois comptes sur un serveur, l'un voyant tout, l'un n'ayant reçu que les films, l'un n'ayant rien reçu, passés par chaque chemin de lecture et d'écriture. Et le banc d'essai repassé en release sur cent mille œuvres inventées : tous les budgets tenus, la lecture de session sur chaque requête coûtant moins d'une milliseconde.

### Le reste du jalon

Fait depuis, en avance sur ce jalon : la moitié serveur du refus de la conversion des couleurs, réglage général sous « Image » dans les paramètres, décoché par défaut. Demandé pour un processeur trop lent pour reconstruire l'image des films HDR d'une collection, plutôt que pour un écran réellement HDR. Le Dolby Vision sans couche de base compatible reste converti dans tous les cas, pour ne jamais laisser une image cassée à l'écran. L'interrupteur pendant la lecture et le réglage par spectateur, pensés pour l'écran plutôt que pour le processeur, sont faits depuis : voir plus bas.

- Écran d'administration : nom du serveur, logo, écran de démarrage, fond et apparence de la page de connexion. **Fait pour le nom, le logo et le fond de la page de connexion** (fond dessiné au choix, ou image déposée qui gagne sur lui). Reste l'écran de démarrage.
- Couleur d'accentuation choisie par l'utilisateur, avec palette dérivée et vérification automatique du contraste. **À moitié fait** : le compte la porte, elle est refusée si ce n'est pas un dièse et six chiffres, ses nuances sont dérivées et la couleur lisible dessus est calculée. L'écran qui la choisit est fait, dans Apparence. **Fait.**
- Modes clair, sombre et automatique, stockés dans les préférences. **Fait** : le compte les porte, le navigateur en garde une copie pour dessiner la porte avant de connaître qui que ce soit.
- CSS personnalisé, au niveau du serveur pour l'administrateur et au niveau de chaque utilisateur, avec possibilité de le désactiver et adresse de secours sans CSS.
- Lecteur avec ses propres commandes à la place de celles du navigateur : barre de progression, volume, plein écran, titre du film et retour, le tout cohérent en fenêtre comme en plein écran, et rejoignant le choix des pistes, la vitesse et l'image dans un coin qui existent déjà autour. Les commandes du navigateur conviennent à une vidéo regardée vite fait : elles ne portent ni le nom de ce qu'on regarde, ni ce qui l'entoure, et elles changent d'un navigateur à l'autre. **Fait.**
- Mode maintenance : message, durée estimée, page dédiée avec le code d'état approprié, accès conservé pour l'administrateur, avertissement des lectures en cours, exemple de configuration nginx.
- Gestion des utilisateurs et de leurs droits : accès par bibliothèque, limite d'âge, téléchargement, suppression, sessions simultanées. Code à quatre chiffres pour les appareils de télévision déjà autorisés. **Voir la section « Comptes, connexion et droits » plus haut** : l'accès par bibliothèque est fait et appliqué partout, et l'écran des comptes existe ; restent le téléchargement, la limite d'âge et le code des télévisions.
- Téléchargement d'un fichier, soumis au droit correspondant.
- **Fait.** Suppression d'une œuvre, avec case décochée par défaut pour effacer aussi le fichier du disque, réservée à l'administrateur, chemin résolu côté serveur et vérifié sous une racine déclarée, entrée au journal d'activité.
- Assistant de première configuration : langue, compte administrateur, mode d'accès. **Fait pour le compte et la langue** : un serveur neuf n'a aucun compte et la première chose qu'il propose est d'en créer un, avec le nom que la personne choisit, et cette porte se referme derrière elle. Reste à y joindre les médiathèques, dont l'explorateur de dossiers, le formulaire et le premier scan existent déjà dans les réglages, et le mode d'accès.
- Écran de choix d'utilisateur avec avatars, désactivable. **Fait** : les comptes sont proposés sur l'écran de connexion, chacun peut s'en retirer, et chacun choisit sa photo de profil dans ses réglages, montrée sur cet écran et dans l'en-tête.
- Liste des appareils connectés avec leur dernière activité et révocation individuelle. **Fait** : voir la section « Comptes, connexion et droits » plus haut.
- Page des bibliothèques affichant l'état d'accès de chaque racine. Les fonctions exigeant l'écriture (suppression sur disque, écriture des fichiers d'accompagnement) sont désactivées avec leur raison quand la racine est en lecture seule.
- **Le HDR gardé tel quel sur un écran qui l'affiche, au choix du spectateur.** **Fait.** Le lecteur demande au navigateur si l'écran est en mode HDR et, codec par codec (HEVC, VP9, AV1 en 10 bits), s'il affiche l'image HDR sur la courbe PQ et sur la courbe HLG, en fichier entier comme en morceaux ; le serveur garde alors le film tel quel au lieu de le convertir. Un **réglage par compte** dans Mes réglages, Lecture, panneau Image : « Automatique » par défaut, « Toujours convertir », « Ne jamais convertir ». Un **choix dans les paramètres du lecteur**, ligne HDR, valable pour ce film seulement et qui relance la lecture là où elle en était ; quand le choix ne change rien (image refaite pour une autre raison, Dolby Vision de profil 5, serveur réglé pour ne jamais convertir), la ligne dit pourquoi au lieu de proposer. Le Dolby Vision de profil 5 est maintenant converti **toujours**, pour tout écran : avant, il ne l'était que parce qu'aucun écran n'était pris pour un écran HDR. La ligne « playback decided » du journal technique dit, pour un film HDR, le choix du spectateur et ce que le navigateur a déclaré afficher. Reste à vérifier sur un vrai écran HDR sous Windows que Chrome, Edge et Brave répondent oui aux questions posées.
- **Longueur du saut des boutons reculer et avancer, réglable par l'utilisateur.** Deux valeurs distinctes, une par bouton, parce que reculer et avancer ne servent pas à la même chose : on recule pour réentendre une réplique, on avance pour passer un générique. **Fait** : réglé par compte dans Mes réglages, Lecture, et suivi par les flèches du clavier.
- **Surveillance en temps réel des médiathèques**, comme l'option « Activer la surveillance en temps réel » de Jellyfin : un interrupteur par médiathèque, et quand il est coché, un fichier ajouté, déplacé, renommé ou supprimé dans ses dossiers est traité aussitôt, sans attendre un scan. Seulement sur les systèmes de fichiers qui préviennent d'un changement ; ailleurs l'interrupteur dit pourquoi il ne peut rien. Demandé par le mainteneur. Jellyfin et Emby le font dans le même genre de conteneur non privilégié. Un film en cours de copie n'est pris qu'une fois sa taille stable depuis un moment, comme le fait Jellyfin, et un renommage ou un déplacement sur le même disque est traité tout de suite.
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

## Jalon 11 : interface principale

État : en cours.

Reprise à zéro de l'accueil et de la navigation, sur une direction visuelle donnée par le mainteneur : l'accessibilité d'Emby et de Jellyfin, avec une identité plus soignée et plus cinématique. Le modèle de base prévoyait presque tout ce que cette interface demande depuis la première migration : favoris, à voir plus tard, collections, listes de lecture, étiquettes, compteurs d'épisodes non vus, droits par compte. L'essentiel du travail moteur est donc de brancher, pas d'inventer.

Côté moteur :

- **La carte enrichie** : sa sorte, sa médiathèque, où en est celui qui regarde, son favori, ses épisodes restants, et de quoi lancer la lecture sans ouvrir la fiche. C'est la fondation de tout le reste, parce que chaque signe du survol est une donnée qui appartient à celui qui regarde.
- **Vu et non vu à la main**, sur un film, une saison et une série entière, compteurs réécrits dans la même transaction.
- **Les favoris** comme vue à part entière, et **À suivre**, qui donne le prochain épisode non commencé de chaque série commencée.
- **L'accueil en modules** au lieu d'une seule rangée : hero, continuer la lecture, à suivre, ajoutés récemment, puis une rangée par sorte de médiathèque réellement présente. Chaque module vide disparaît, et tout est borné aux médiathèques que le compte a le droit de voir.
- **Épinglé** par l'administrateur, et **suggestions** définies honnêtement (voir le README des décisions).

Côté interface :

- Le jeu d'icônes, dessiné à la main comme celui du lecteur, sans bibliothèque externe.
- Le header : catégories déduites des médiathèques réelles, recherche avec filtre de périmètre, cast et notifications visibles mais grisés avec leur infobulle.
- La carte et son survol : agrandissement sans déplacer les voisines, lecture au centre, vu en haut à droite, favori et menu en bas à droite, barre de progression, badge d'épisodes restants.
- Le hero, module désactivable de cinq éléments au maximum, défilement lent qui s'arrête dès qu'on le touche.
- Le menu contextuel préparant toutes les actions prévues, grisées quand elles ne sont pas câblées et masquées quand le compte n'y a pas droit.
- La passe de finition, menée en regardant la direction visuelle à côté de l'écran : le bandeau de catégories sous le hero, le chevron et le « Tout voir » des rangées adossées à une grille, les pastilles techniques et la durée en heures du hero, le temps restant et la ligne de saison sur les cartes de reprise, l'icône de chaque catégorie, la pastille de raccourci clavier et la pastille du compte dans la barre du haut.

Ce qui n'est pas dans ce jalon : l'écran de personnalisation de l'accueil (l'architecture doit le permettre, il ne se développe pas encore), l'interface d'administration séparée, et l'adaptation au téléphone, qui ne doit simplement pas être rendue impossible.

Demandé par le mainteneur pendant ce jalon et remis à son propre chantier :

- **L'ordre du bandeau de catégories choisi par la personne.** Fait : un réglage de compte, dans les réglages, que le bandeau et les rangées par sorte suivent tous les deux (voir le README des décisions).
- **Changer son nom d'utilisateur. Fait** : depuis son profil, et par l'administrateur depuis la page Utilisateurs ; les appareils restent connectés.

## Après la V0.1

- Séries : **corriger un générique à la main** depuis la fiche d'un épisode, ce qui passe devant ce que le fichier dit et devant ce que l'écoute a trouvé. La place est déjà faite dans le modèle, il manque l'écran.
- Séries : **ranger** à nouveau des épisodes déjà rangés quand les règles s'améliorent. Le **nom** d'une série encore sans nom est désormais relu à chaque scan, comme celui d'un film, et une série mal nommée se répare donc toute seule. Ce qui reste est le rangement lui-même : un épisode déjà accroché à une saison n'est jamais rattaché ailleurs, parce que son fichier n'a pas bougé, donc un épisode mal placé le reste. La seule façon d'en sortir est de retirer la médiathèque et de la remettre, ce qui ne touche à aucun fichier mais fait tout redemander au fournisseur.
- Séries : l'affiche d'une saison ou la vignette d'un épisode qui n'est pas arrivée le jour de l'identification n'est redemandée que le jour où la série l'est à nouveau. Un film, lui, a sa passe de rattrapage ; une saison ne peut pas l'avoir telle quelle, puisqu'elle ne se demande jamais toute seule mais par la série au-dessus d'elle.
- Animés : un catalogue spécialisé en second fournisseur, dans le chantier des fournisseurs multiples. Les noms de sortie des animés et la numérotation absolue sont lus et replacés dans les saisons du fournisseur actuel (voir le README des décisions). Reste à faire : un épisode arrivé après l'identification de sa série est bien placé mais pas encore décrit, comme pour les séries, et un numéro compté sur toute la série à l'intérieur d'un dossier de saison est lu comme compté dans cette saison.
- Plusieurs fournisseurs de métadonnées : un second catalogue interrogé quand le premier ne connaît pas un film, des règles claires pour départager deux réponses, et la provenance restant visible champ par champ. Constaté en conditions réelles : un téléfilm rattaché à une série existe chez le fournisseur actuel du côté des séries et pas du côté des films, donc aucune recherche de film ne le trouvera jamais, alors qu'un catalogue construit sur les données IMDb le classe comme film. Les autres serveurs y arrivent parce qu'ils ont un second catalogue sous la main, pas parce que leur lecture des noms est meilleure.
- Photos et vidéos perso : type de médiathèque en place (voir le README des décisions), navigation par dossiers, images tirées des fichiers, visionneuse de photos. Reste à faire : la date de prise de vue lue dans la photo pour trier et regrouper par date, le diaporama, et les formats que le navigateur n'affiche pas (HEIC des téléphones récents), aujourd'hui ignorés.
- Bibliothèque musicale : modèle artiste, album, morceau, fournisseur MusicBrainz, navigation dédiée, listes de lecture.
- Normalisation audio : c'est la vraie réponse au niveau sonore, et le gain de repliage appliqué aujourd'hui à toutes les pistes tient la place en attendant. Mesure de sonie au scan, application au gain à la lecture, modes morceau et album, compression de plage dynamique pour les films.
- Clients natifs : un projet Android, base commune (API, session, cache, lecteur), interface télévision d'abord, interface téléphone ensuite.
- Émissions : type de bibliothèque à part entière (documentaires et programmes de télévision), réutilisant le modèle série, saison, épisode.
- Versions numérotées et **mise à jour depuis l'interface**. Une fois la mise à jour terminée, chaque page ouverte doit se recharger d'elle-même : sans cela, un onglet resté ouvert garde l'ancienne interface, qui peut réclamer au serveur des fichiers ou des réponses qui n'existent plus. Le script garde aujourd'hui les fichiers de l'interface précédente pour une mise à jour, ce qui couvre le cas le plus courant, pas tous.
- Import ponctuel de l'historique de visionnage et des favoris depuis une installation Jellyfin existante.
- Recherche et téléchargement de sous-titres en ligne, sur demande explicite.
- Contrôle à distance d'une session de lecture depuis un autre appareil, sans priorité.
- Interface ambitieuse, surveillance des dossiers en temps réel.
