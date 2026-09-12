# Plan par jalons

Chaque jalon est compilable et testable seul dans le LXC, avec un résultat visible pour le mainteneur. On ne passe au suivant que lorsque le précédent fonctionne en conditions réelles. L'état est tenu à jour ici.

Les fonctionnalités voulues et leurs conséquences sont détaillées dans [04-fonctionnalites.md](04-fonctionnalites.md).

Légende : à faire, en cours, terminé.

## Jalon 0 : socle

État : à faire.

- Workspace Cargo avec les crates `core`, `config`, `database`, `ffmpeg`, `app`, `server`, `melyxar` (les autres arrivent quand elles servent).
- Configuration TOML (`/etc/melyxar/melyxar.toml`) : chemins des données, du cache, des transcodages, du binaire FFmpeg, port 2100, mode d'accès (HTTP ou HTTPS) prévu dès maintenant, bibliothèques déclarées.
- Base SQLite ouverte en WAL, migrations appliquées au démarrage. Première migration avec :
  - utilisateur, préférences utilisateur, jeton de session ;
  - paramètres du serveur (nom, logo, fonds, CSS global, mode maintenance) ;
  - bibliothèque avec son **type explicite** (films, séries, musique) ;
  - tronc commun œuvre, source média, piste, **séparé des métadonnées spécifiques au domaine** ;
  - colonnes de **sonie EBU R128** sur les pistes audio, renseignées plus tard ;
  - vidéos annexes (bandes annonces), favoris, état de progression explicite, compteurs agrégés.
- Répertoire des fichiers envoyés par l'administrateur, dans les données et non dans le cache.
- Journalisation structurée avec temps par requête, censure des noms de médias.
- Commande `melyxar doctor` : version de FFmpeg et accélérations, accès à `/dev/dri`, permissions sur chaque racine, mode WAL, tailles.
- Script de mise à jour pour le LXC (récupérer, compiler en priorité basse, migrer, redémarrer) et unité systemd.
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
- Téléchargement des affiches et fonds, génération des tailles fixes en WebP, couleur dominante, URL avec empreinte.
- Résultat visible : les œuvres ont titre, année, synopsis et affiches dans la base et le cache.

## Jalon 3 : API et interface minimale

État : à faire.

- Routes de navigation : liste de cartes paginée par curseur, fiche, images, recherche par préfixe.
- Route publique d'identité visuelle (nom et logo), sans authentification et sans divulgation.
- Erreurs renvoyées sous forme de code et de données, jamais de phrase toute faite.
- Spécification OpenAPI générée, client TypeScript généré.
- Interface React : grille virtualisée, fiche, cache des réponses, images adaptées. Disposition inspirée d'Emby pour l'accueil et la fiche.
- **Jetons de thème et internationalisation dès le premier composant** : aucune couleur ni chaîne en dur.
- Lu et non lu, marquage manuel, favoris.
- Résultat visible : la bibliothèque s'affiche dans Brave, navigation instantanée sur les 50 films.

## Jalon 4 : lecture Direct Play

État : à faire.

- Crate `playback` avec la décision pure et ses raisons.
- Profil de capacités construit par le client web et envoyé au serveur.
- Fichier servi avec requêtes Range, balise vidéo native.
- Progression enregistrée en temps réel, copie locale côté client, position horodatée jamais écrasée par une plus ancienne.
- Courbe de volume vérifiée à l'oreille sur toute la course.
- Résultat visible : un MP4 H.264 AAC se lit dans Brave, le journal explique pourquoi Direct Play a été choisi, fermer l'onglet en pleine lecture ne perd pas la position.

## Jalon 5 : remux et transcodage HLS

État : à faire.

- Crate `streaming` : sessions, playlist générée par le serveur, segments à la demande, battement de cœur, nettoyage.
- Remux vers HLS fMP4, transcodage audio vers AAC, transcodage vidéo logiciel vers H.264, sous-titres texte vers WebVTT.
- Seeking par relance de FFmpeg à la position demandée.
- État de préparation de lecture exposé par étapes nommées, avec progression réelle sur la production du premier segment et la mise en tampon.
- Lecture des bandes annonces, locales et distantes.
- Limite de sessions simultanées, arrêt propre sur SIGTERM, balayage au démarrage.
- Résultat visible : le premier jalon utile du document 01. Un MKV avec audio EAC3 se lit dans Brave, un seek à 80 % fonctionne, aucun FFmpeg ne survit à la fermeture de l'onglet.

## Jalon 6 : accélération matérielle

État : à faire.

- Backend QSV / VAAPI pour l'Intel Arc A380 : détection, test réel, repli logiciel.
- Tonemapping HDR sur la carte.
- Résultat visible : un film 4K HDR se lit en SDR avec moins d'un cœur de processeur utilisé.

## Jalon 7 : réactivité mesurée et tableau de bord

État : à faire.

- Générateur de bibliothèque synthétique (10 000, 50 000, 100 000 œuvres).
- Script de charge et comparaison aux budgets du document 02, avec et sans scan.
- Tableau de bord d'administration en temps réel : sessions de lecture et leurs décisions, charge processeur et mémoire, carte graphique, tâches de fond, file d'écriture, espace disque, dernières erreurs. Flux d'événements arrêté quand personne ne regarde.
- Export de diagnostic en un clic.
- Résultat visible : les budgets sont tenus à 100 000 œuvres, chiffres à l'appui, et le tableau de bord montre l'activité en direct.

## Jalon 8 : personnalisation et administration

État : à faire.

- Écran d'administration : nom du serveur, logo, écran de démarrage, fond et apparence de la page de connexion.
- Couleur d'accentuation choisie par l'utilisateur, avec palette dérivée et vérification automatique du contraste.
- Modes clair, sombre et automatique, stockés dans les préférences.
- CSS personnalisé, au niveau du serveur pour l'administrateur et au niveau de chaque utilisateur, avec possibilité de le désactiver et adresse de secours sans CSS.
- Mode maintenance : message, durée estimée, page dédiée avec le code d'état approprié, accès conservé pour l'administrateur, avertissement des lectures en cours, exemple de configuration nginx.
- Résultat visible : le serveur ne ressemble plus à l'installation par défaut, et une maintenance annoncée s'affiche proprement.

## Jalon 9 : accès et chiffrement

État : à faire.

- Quatre options présentées honnêtement : derrière un reverse proxy, certificat auto-signé, certificat fourni par l'administrateur, certificat reconnu obtenu automatiquement avec un nom de domaine.
- Service HTTPS par le serveur lui-même pour les options concernées.
- Résultat visible : l'administrateur choisit son mode dans l'interface et sait ce que chacun implique.

## Après la V0.1

- Séries : modèle série, saison, épisode, compteurs visibles, navigation dédiée.
- Bibliothèque musicale : modèle artiste, album, morceau, fournisseur MusicBrainz, navigation dédiée, listes de lecture.
- Normalisation audio : mesure de sonie au scan, application au gain à la lecture, modes morceau et album, compression de plage dynamique pour les films.
- Plusieurs utilisateurs avec écran de connexion complet et gestion des droits.
- Clients natifs : un projet Android, base commune (API, session, cache, lecteur), interface télévision d'abord, interface téléphone ensuite.
- Interface ambitieuse, surveillance des dossiers en temps réel.
