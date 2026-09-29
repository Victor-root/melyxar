# Musique

Chantier à venir. Rien n'est codé : ce document fixe la règle qui le guide, dit ce qui existe déjà et ce qui manque, et garde les questions encore ouvertes. Il se met à jour à mesure que les décisions se prennent.

## Principe directeur

**La base est le fonctionnement d'Emby et de Jellyfin sur la musique**, pas la façon dont le mainteneur range ses propres fichiers. Sa manière de faire servira à la fin, pour vérifier que la base tient sur un cas réel.

- Les étiquettes intégrées aux fichiers font foi (artiste, album, numéro de piste) ; le dossier ne sert que de secours quand elles manquent.
- Les fournisseurs en ligne sont optionnels et réglables par médiathèque. Tout désactivé, la musique marche avec ce qui est sur le disque.
- Les pochettes locales (`cover.jpg`, `folder.jpg` et leurs semblables) sont prises automatiquement, sans réglage.
- Squelette de l'écran repris de Jellyfin : onglets Albums, Suggestions, Artistes d'albums, Artistes, Listes de lecture, Chansons et Genres ; grille de pochettes ; barre de lettres ; lecture et mélange en tête de liste. Le style reste celui de Melyxar.

## Règle d'isolation

**La musique est un chantier à part, isolé du reste autant que possible.** Le mainteneur ne lit pas le code et ne peut pas tout vérifier à l'œil : la meilleure protection est une cloison. Si la musique a un défaut, il reste dans la musique, et les films, les séries, les photos et l'accueil continuent de marcher.

- **Dans l'interface** : un lecteur audio dédié, dans son propre dossier, qui n'utilise pas le moteur du lecteur vidéo. Il peut se servir des briques neutres (formatage du temps, mémoire du volume, mots d'erreur, appels au serveur), mais elles sont alors sorties du dossier du lecteur vidéo vers un endroit neutre, au lieu d'être importées depuis lui.
- **Sens unique** : la musique peut dépendre des briques neutres ; le lecteur vidéo, l'accueil et le reste ne dépendent jamais de la musique.
- **Côté serveur** : un chemin dédié à la musique, séparé de la fabrication des films en morceaux. La décision de lecture des films (crate `playback`) et les sessions découpées (crate `streaming`) ne sont pas modifiées pour la musique. Ce qui est propre à la musique (lecture des étiquettes, regroupement en artistes et albums) vit dans son propre module ou sa propre crate.
- **Un défaut de la musique ne peut atteindre ni la lecture des films, ni l'accueil, ni le reste.**

Pourquoi pas un lecteur commun : la musique joue autrement (pas de session découpée, pas d'image, une file d'attente qui enchaîne sans coupure, une barre qui reste en bas pendant qu'on navigue), et toucher au lecteur vidéo, qui marche bien, ferait courir un risque à tout le reste pour économiser peu de code.

## Ce qui existe déjà

- Le modèle de données : les œuvres de sorte artiste, album et morceau, avec parent et numéro de piste, et les colonnes de sonie (intégrée, crête, plage) sur les pistes audio.
- La sorte de médiathèque « Musique » : dans le moteur, dans l'administration et dans l'ordre de l'accueil, éteinte.
- Les listes de lecture, les images, les personnes, les tâches de fond.
- L'analyse des fichiers, qui écarte déjà la pochette intégrée à un fichier audio.
- Un précédent de nouvelle sorte de médiathèque : les photos et vidéos perso.

## Ce qui reste à faire

Ordre proposé, chaque étape livrable seule :

1. **Le scan** : comprendre `Artiste/Album/01 - Titre.flac` et lire les étiquettes intégrées (titre, artiste, album, numéro de piste, année, disque).
2. **L'affichage** : pages artiste, album et morceau, navigation pensée pour une discothèque de plus de 100 000 morceaux.
3. **Un lecteur simple** : lire un morceau, et le suivant de l'album.
4. **Le lecteur complet** : barre persistante qui reste pendant la navigation, file d'attente, aléatoire, répétition, enchaînement sans coupure.
5. **Les listes de lecture** avec des morceaux.
6. **La normalisation du volume** : mesure au scan, gain à la lecture, modes morceau et album.
7. **Les informations en ligne** : MusicBrainz et les pochettes.

## Décisions prises avec le mainteneur

- **Tous les formats audio courants se lisent**, y compris les plus rares (ALAC, WMA, APE, WavPack) : ce que le navigateur ne lit pas tel quel est converti en fichier audio simple par le chemin serveur de la musique.
- **Un lecteur en bas de l'écran, comme Emby et Jellyfin** : pochette, titre et artiste, précédent, lecture, arrêt, suivant, temps, volume, aléatoire, répétition, favori et file d'attente. Il reste pendant qu'on navigue. Seule l'ergonomie est reprise, l'apparence est celle de Melyxar.
- **Page d'un album comme chez Jellyfin** : pochette, nom, artiste, nombre de pistes, boutons Lire, Aléatoire, favori et menu, puis la liste des morceaux avec leur durée, un favori et un menu par ligne.
- **Le volume laisse toujours le choix** : un même niveau pour tous les morceaux, ou les écarts d'un album gardés.
- **L'écoute en arrière-plan est prévue d'office** : écran verrouillé, onglet en arrière-plan, touches multimédia du clavier et du téléphone.
- **L'accueil a une rangée dédiée à la musique**, qui ne passe ni dans « Continuer la lecture » ni dans « À suivre ». Ses propres rangées (écoutés récemment, ajoutés récemment) vivent dans la médiathèque de musique.
- **Les paroles sont à mettre en place**, défilant en rythme quand elles portent l'heure de chaque ligne. Sources, de la plus locale à la plus distante : paroles intégrées au fichier, fichier `.lrc` à côté du morceau, puis LRCLIB (gratuit, sans clé) si le fournisseur est activé, réglable par médiathèque.
- **LRCLIB vérifié le 29 septembre 2026** : sans compte ni clé, une requête `GET /api/get` avec artiste, titre, album et durée rend les paroles seules et les paroles synchronisées (une heure par ligne, au format `[0:07.78]`), et une réponse 404 `TrackNotFound` quand le morceau est inconnu. La durée n'a pas besoin d'être exacte à la seconde. Le service répond parfois 503 « serveur occupé » : la recherche ne doit jamais bloquer ni la lecture ni le scan, elle réessaie plus tard. Aucune limite de débit ni condition d'usage n'est publiée : s'identifier par un `User-Agent`, ne demander qu'une fois par morceau et garder la réponse, y compris l'absence de réponse.

## Questions ouvertes

À trancher avec le mainteneur avant de coder l'étape concernée. Les réponses sont ajoutées ici, puis au README des décisions.

Aucune pour le moment.
