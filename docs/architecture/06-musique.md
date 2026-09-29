# Musique

Chantier à venir. Rien n'est codé : ce document fixe la règle qui le guide, dit ce qui existe déjà et ce qui manque, et garde les questions encore ouvertes. Il se met à jour à mesure que les décisions se prennent.

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

## Questions ouvertes

À trancher avec le mainteneur avant de coder l'étape concernée. Les réponses sont ajoutées ici, puis au README des décisions.

- Comment sont rangés les fichiers : un dossier par artiste puis par album, ou tout à plat ? Faut-il se fier aux étiquettes intégrées, aux noms de dossiers, ou aux deux ?
- Quels formats faut-il lire au minimum (MP3, FLAC, AAC, OGG, OPUS, ALAC, WMA, formats sans perte plus rares) ?
- Les pochettes : intégrées aux fichiers, fichier `cover.jpg` dans le dossier, ou récupérées en ligne ?
- Les paroles : hors sujet, ou une place prévue plus tard ?
- La barre de lecture : un mini lecteur en bas, plus une page « en cours de lecture » en grand ?
- Le volume : un même niveau pour tous les morceaux, ou conserver les écarts d'un album ?
- Un morceau joué compte-t-il dans « Continuer la lecture » et dans l'historique, ou la musique a-t-elle son propre historique ?
- Faut-il pouvoir écouter écran verrouillé ou onglet en arrière-plan, avec les touches multimédia du clavier et du téléphone ?
