# Plan par jalons

Chaque jalon est compilable et testable seul dans le LXC, avec un résultat visible pour le mainteneur. On ne passe au suivant que lorsque le précédent fonctionne en conditions réelles. L'état est tenu à jour ici.

Les fonctionnalités voulues et leurs conséquences sont détaillées dans [04-fonctionnalites.md](04-fonctionnalites.md).

Légende : à faire, en cours, terminé.

## Jalon 0 : socle

État : à faire.

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
  - **journal d'activité**, indexé par date et purgeable.
- Répertoire des fichiers envoyés par l'administrateur, dans les données et non dans le cache.
- Journalisation structurée avec temps par requête, censure des noms de médias.
- Commande `melyxar doctor` : version de FFmpeg et accélérations, accès à `/dev/dri`, permissions sur chaque racine, mode WAL, tailles.
- **Script shell d'installation et de mise à jour**, aux conventions du dépôt `Proxmox-Tools` du mainteneur (bilingue intégré, couleurs 256 niveaux désactivables, bannière, indicateur animé, encadrés, menu numéroté, sauvegarde avant modification, aucune dépendance) : vérification du système, outils, utilisateur système et répertoires, récupération et compilation en priorité basse, configuration, service, migrations, redémarrage, adresse à ouvrir. Sert aussi à la mise à jour et à la désinstallation.
- Unité systemd.
- Résultat visible : le service démarre, `ip:2100/api/v1/system/info` répond, `melyxar doctor` affiche un diagnostic lisible.

## Jalon 1 : scan et analyse

État : à faire.

- Crates `library`, `media_probe`, `jobs`.
- Scan incrémental d'une bibliothèque de films : ajouts, mises à jour, marquage absent, protection contre une racine injoignable.
- Analyse ffprobe de chaque fichier, pistes stockées avec codecs, HDR, langues.
- Détection des bandes annonces locales à côté des films.
- Analyse des noms de fichiers (titre, année) avec tests sur des dizaines de cas.
- Tâches de fond avec parallélisme borné, priorité basse, annulation.
- Résultat visible : après un scan de la bibliothèque de test (environ 50 films), la base contient les œuvres, sources et pistes ; le journal montre le déroulement sans nom complet de fichier.

## Jalon 2 : métadonnées et images

État : à faire.

- Crate `metadata` avec le fournisseur TMDb derrière un trait.
- Identification des films, identifiants externes, provenance des champs, liens de bandes annonces.
- Personnes (acteurs, réalisateurs) avec photos, collections officielles, films similaires, classification d'âge.
- Textes stockés par langue, français prioritaire et repli anglais.
- Correction manuelle de l'identification depuis l'interface : recherche par titre et année, saisie directe d'un identifiant, choix parmi les propositions illustrées.
- Téléchargement des affiches et fonds, génération des tailles fixes en WebP, couleur dominante, URL avec empreinte.
- Résultat visible : les œuvres ont titre, année, synopsis et affiches dans la base et le cache, et un film mal identifié se corrige en quelques clics.

## Jalon 3 : API et interface minimale

État : à faire.

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
- Interface React : grille virtualisée, fiche, cache des réponses, images adaptées. Disposition inspirée d'Emby pour l'accueil et la fiche.
- **Jetons de thème et internationalisation dès le premier composant** : aucune couleur ni chaîne en dur.
- Page d'accueil : bannière en haut, reprendre la lecture, récemment ajouté par bibliothèque, suggestions. Sections réordonnables et masquables par l'utilisateur.
- Fiche complète : distribution cliquable menant à la filmographie, studios, films similaires, versions et pistes disponibles.
- Section dédiée aux sagas : liste des coffrets, chacun ouvrable sur ses films.
- Lu et non lu, marquage manuel, favoris, étiquettes, listes de lecture.
- Modification manuelle d'une fiche avec verrouillage des champs.
- Résultat visible : la bibliothèque s'affiche dans Brave, navigation instantanée sur les 50 films.

## Jalon 4 : lecture Direct Play

État : à faire.

- Crate `playback` avec la décision pure et ses raisons.
- Profil de capacités construit par le client web et envoyé au serveur.
- Fichier servi avec requêtes Range, balise vidéo native.
- Progression enregistrée en temps réel, copie locale côté client, position horodatée jamais écrasée par une plus ancienne.
- Reprise directe avec bouton distinct pour repartir du début.
- Courbe de volume vérifiée à l'oreille sur toute la course.
- Mémoire des langues audio et sous-titres, par utilisateur puis par série.
- Vitesse de lecture, image dans l'image, raccourcis clavier.
- Résultat visible : un MP4 H.264 AAC se lit dans Brave, le journal explique pourquoi Direct Play a été choisi, fermer l'onglet en pleine lecture ne perd pas la position.

## Jalon 5 : remux et transcodage HLS

État : à faire.

- Crate `streaming` : sessions, playlist générée par le serveur, segments à la demande, battement de cœur, nettoyage.
- Remux vers HLS fMP4, transcodage audio vers AAC, transcodage vidéo logiciel vers H.264, sous-titres texte vers WebVTT.
- Seeking par relance de FFmpeg à la position demandée.
- État de préparation de lecture exposé par étapes nommées, avec progression réelle sur la production du premier segment et la mise en tampon.
- Lecture des bandes annonces, locales et distantes.
- Apparence des sous-titres réglable : taille, couleur, contour, fond, position.
- Limite de sessions simultanées, arrêt propre sur SIGTERM, balayage au démarrage.
- Résultat visible : le premier jalon utile du document 01. Un MKV avec audio EAC3 se lit dans Brave, un seek à 80 % fonctionne, aucun FFmpeg ne survit à la fermeture de l'onglet.

## Jalon 6 : accélération matérielle

État : à faire.

- Backend QSV / VAAPI pour l'Intel Arc A380 : détection, test réel, repli logiciel.
- Tonemapping HDR sur la carte.
- Vignettes de chapitres et aperçu de la barre de lecture en planches, **toujours converties en SDR quand la source est HDR** pour éviter des images délavées. Intervalle et résolution configurables, génération activable par bibliothèque.
- Résultat visible : un film 4K HDR se lit en SDR avec moins d'un cœur de processeur utilisé, et ses vignettes sont en couleurs correctes.

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

- Écran d'administration : nom du serveur, logo, écran de démarrage, fond et apparence de la page de connexion.
- Couleur d'accentuation choisie par l'utilisateur, avec palette dérivée et vérification automatique du contraste.
- Modes clair, sombre et automatique, stockés dans les préférences.
- CSS personnalisé, au niveau du serveur pour l'administrateur et au niveau de chaque utilisateur, avec possibilité de le désactiver et adresse de secours sans CSS.
- Mode maintenance : message, durée estimée, page dédiée avec le code d'état approprié, accès conservé pour l'administrateur, avertissement des lectures en cours, exemple de configuration nginx.
- Gestion des utilisateurs et de leurs droits : accès par bibliothèque, limite d'âge, téléchargement, suppression, sessions simultanées. Code à quatre chiffres pour les appareils de télévision déjà autorisés.
- Téléchargement d'un fichier, soumis au droit correspondant.
- Suppression d'une œuvre, avec case décochée par défaut pour effacer aussi le fichier du disque, réservée à l'administrateur, chemin résolu côté serveur et vérifié sous une racine déclarée, entrée au journal d'activité.
- Assistant de première configuration : langue, compte administrateur, bibliothèques ajoutées en parcourant l'arborescence du serveur, langue des métadonnées et clé du fournisseur, mode d'accès, premier scan. S'ouvre tant que la configuration initiale n'est pas terminée et saute ce que le script d'installation a déjà réglé.
- Écran de choix d'utilisateur avec avatars, désactivable.
- Statistiques personnelles : temps de visionnage, films vus sur une période, genres préférés.
- Notification d'une version plus récente publiée sur GitHub, sans mise à jour automatique, vérification désactivable.
- Sauvegarde automatique quotidienne de la base et des fichiers envoyés, quelques copies conservées.
- Résultat visible : le serveur ne ressemble plus à l'installation par défaut, et une maintenance annoncée s'affiche proprement.

## Jalon 9 : accès et chiffrement

État : à faire.

- Quatre options présentées honnêtement : derrière un reverse proxy, certificat auto-signé, certificat fourni par l'administrateur, certificat reconnu obtenu automatiquement avec un nom de domaine.
- Service HTTPS par le serveur lui-même pour les options concernées.
- Résultat visible : l'administrateur choisit son mode dans l'interface et sait ce que chacun implique.

## Après la V0.1

- Séries : modèle série, saison, épisode, compteurs visibles, navigation dédiée, enchaînement automatique de l'épisode suivant, saut d'intro et de générique (chapitres du fichier, détection par comparaison des empreintes sonores d'une saison, correction manuelle).
- Animés : fournisseur de métadonnées adapté et numérotation propre au domaine.
- Bibliothèque musicale : modèle artiste, album, morceau, fournisseur MusicBrainz, navigation dédiée, listes de lecture.
- Normalisation audio : mesure de sonie au scan, application au gain à la lecture, modes morceau et album, compression de plage dynamique pour les films.
- Plusieurs utilisateurs avec écran de connexion complet et gestion des droits.
- Clients natifs : un projet Android, base commune (API, session, cache, lecteur), interface télévision d'abord, interface téléphone ensuite.
- Recherche et téléchargement de sous-titres en ligne, sur demande explicite.
- Contrôle à distance d'une session de lecture depuis un autre appareil, sans priorité.
- Interface ambitieuse, surveillance des dossiers en temps réel.
