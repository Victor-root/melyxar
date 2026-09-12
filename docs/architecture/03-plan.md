# Plan par jalons

Chaque jalon est compilable et testable seul dans le LXC, avec un résultat visible pour le mainteneur. On ne passe au suivant que lorsque le précédent fonctionne en conditions réelles. L'état est tenu à jour ici.

Légende : à faire, en cours, terminé.

## Jalon 0 : socle

État : à faire.

- Workspace Cargo avec les crates `core`, `config`, `database`, `ffmpeg`, `app`, `server`, `melyxar` (les autres arrivent quand elles servent).
- Configuration TOML (`/etc/melyxar/melyxar.toml`) avec chemins des données, du cache, des transcodages, du binaire FFmpeg, port 2100, bibliothèques déclarées.
- Base SQLite ouverte en WAL, migrations appliquées au démarrage, première migration avec les tables utilisateur, bibliothèque, œuvre, source média, piste.
- Utilisateur par défaut et jeton de session.
- Journalisation structurée avec temps par requête, censure des noms de médias.
- Commande `melyxar doctor` : version de FFmpeg et accélérations, accès à `/dev/dri`, permissions sur chaque racine, mode WAL, tailles.
- Script de mise à jour pour le LXC (récupérer, compiler en priorité basse, migrer, redémarrer) et unité systemd.
- Résultat visible : le service démarre, `ip:2100/api/v1/system/info` répond, `melyxar doctor` affiche un diagnostic lisible.

## Jalon 1 : scan et analyse

État : à faire.

- Crates `library`, `media_probe`, `jobs`.
- Scan incrémental d'une bibliothèque de films : ajouts, mises à jour, marquage absent, protection contre une racine injoignable.
- Analyse ffprobe de chaque fichier, pistes stockées avec codecs, HDR, langues.
- Analyse des noms de fichiers (titre, année) avec tests sur des dizaines de cas.
- Tâches de fond avec parallélisme borné, priorité basse, annulation.
- Résultat visible : après un scan de la bibliothèque de test (environ 50 films), la base contient les œuvres, sources et pistes ; le journal montre le déroulement sans nom complet de fichier.

## Jalon 2 : métadonnées et images

État : à faire.

- Crate `metadata` avec le fournisseur TMDb derrière un trait.
- Identification des films, identifiants externes, provenance des champs.
- Téléchargement des affiches et fonds, génération des tailles fixes en WebP, couleur dominante, URL avec empreinte.
- Résultat visible : les œuvres ont titre, année, synopsis et affiches dans la base et le cache.

## Jalon 3 : API et interface minimale

État : à faire.

- Routes de navigation : liste de cartes paginée par curseur, fiche, images, recherche par préfixe.
- Spécification OpenAPI générée, client TypeScript généré.
- Interface React : grille virtualisée, fiche, cache des réponses, images adaptées. Volontairement moche, mais avec les jetons de thème en place.
- Résultat visible : la bibliothèque s'affiche dans Brave, navigation instantanée sur les 50 films.

## Jalon 4 : lecture Direct Play

État : à faire.

- Crate `playback` avec la décision pure et ses raisons.
- Profil de capacités construit par le client web et envoyé au serveur.
- Fichier servi avec requêtes Range, balise vidéo native.
- Progression de lecture enregistrée.
- Résultat visible : un MP4 H.264 AAC se lit dans Brave, le journal explique pourquoi Direct Play a été choisi.

## Jalon 5 : remux et transcodage HLS

État : à faire.

- Crate `streaming` : sessions, playlist générée par le serveur, segments à la demande, battement de cœur, nettoyage.
- Remux vers HLS fMP4, transcodage audio vers AAC, transcodage vidéo logiciel vers H.264, sous-titres texte vers WebVTT.
- Seeking par relance de FFmpeg à la position demandée.
- Limite de sessions simultanées, arrêt propre sur SIGTERM, balayage au démarrage.
- Résultat visible : le premier jalon utile du document 01. Un MKV avec audio EAC3 se lit dans Brave, un seek à 80 % fonctionne, aucun FFmpeg ne survit à la fermeture de l'onglet.

## Jalon 6 : accélération matérielle

État : à faire.

- Backend QSV / VAAPI pour l'Intel Arc A380 : détection, test réel, repli logiciel.
- Tonemapping HDR sur la carte.
- Résultat visible : un film 4K HDR se lit en SDR avec moins d'un cœur de processeur utilisé.

## Jalon 7 : réactivité mesurée

État : à faire.

- Générateur de bibliothèque synthétique (10 000, 50 000, 100 000 œuvres).
- Script de charge et comparaison aux budgets du document 02, avec et sans scan.
- Page « Diagnostic » dans l'interface, export de diagnostic en un clic.
- Résultat visible : les budgets sont tenus à 100 000 œuvres, chiffres à l'appui.

## Après la V0.1

Séries, plusieurs utilisateurs avec écran de connexion, reverse proxy nginx et HTTPS, surveillance des dossiers, interface ambitieuse, clients Android et TV.
