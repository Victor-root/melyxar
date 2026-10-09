# Musique

Chantier livré dans son ensemble. Ce document fixe la règle qui le guide, dit ce qui a été fait étape par étape, les décisions prises avec le mainteneur et les limites connues.

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

## Organisation du code

Le code de la musique doit être bien séparé et facile à maintenir. Les règles du projet s'appliquent à la lettre, avec en plus la cloison décrite plus haut.

- **Une responsabilité par fichier, dite en une phrase.** Le code neuf va dans le fichier qui porte sa responsabilité, jamais dans celui qu'on était en train de modifier. On découpe quand des fonctions sans rapport se croisent, pas pour le nombre de lignes.
- **Les couches existantes, sans exception** : le SQL de la musique dans `database` uniquement, la logique métier dans `app`, les routes dans `server` sans logique, chacune dans un module à elle. La direction des dépendances entre crates reste unique.
- **Les calculs purs vivent dans une brique basse, avec leurs tests** : regroupement des morceaux en albums et artistes, lecture d'un disque et d'une piste, rendu d'un modèle de renommage. Pas dans l'orchestrateur du scan, pas dans un composant d'interface.
- **Les étiquettes des fichiers ont leur propre brique** (`lofty` y est enfermée), qui ne dépend d'aucune autre crate du serveur : la lecture d'un côté, l'écriture de l'autre, jamais mélangées. Le reste du serveur ne connaît pas `lofty`.
- **Dans l'interface** : un dossier `web/src/music/` pour les pages et le lecteur, ses styles dans un fichier `styles/music.css` chargé par `styles/index.css`, ses textes sous le préfixe `music.` en anglais puis en français. Le lecteur audio, la file d'attente, les paroles, le gestionnaire d'étiquettes et les pages de navigation sont des fichiers distincts.
- **Pas de duplication** : avant d'écrire une aide, on cherche si elle existe. Les briques neutres partagées (temps, volume, appels au serveur) sont sorties à un endroit neutre au lieu d'être copiées.
- **Les tests suivent le code qu'ils testent** : un test d'intégration sur base migrée pour chaque requête SQL de la musique, Vitest pour tout calcul de l'interface, la suite complète et `clippy` pour tout ce qui touche le moteur.
- **Le plan (`03-plan.md`) est mis à jour à chaque étape terminée.** Les noms exacts de crates et de modules sont fixés à l'ouverture du chantier, en respectant ces règles.

## Ce qui existe déjà

- Le modèle de données : les œuvres de sorte artiste, album et morceau. Un morceau pend sous son album, rangé par son numéro de piste ; un album et un artiste ne pendent sous rien, puisqu'un album peut être celui de plusieurs artistes. Ce qui n'appartient qu'à la musique vit dans ses propres tables (migration `0071_music.sql`) : le disque d'un morceau, à qui est un album (et s'il s'agit d'une compilation), et qui est crédité sur un morceau (`artist`) ou un album (`album_artist`). Les colonnes de sonie (intégrée, crête, plage) sont sur les pistes audio depuis la première migration.
- Morceaux, albums et artistes sont marqués « à eux » (`own`), comme les photos et vidéos perso : nommés par leurs fichiers, ils n'attendent aucun catalogue tant que la musique n'a pas le sien.
- La sorte de médiathèque « Musique » : dans le moteur, dans l'administration et dans l'ordre de l'accueil, éteinte dans l'interface tant que l'affichage n'existe pas.
- Les listes de lecture, les images, les tâches de fond.
- Un précédent de nouvelle sorte de médiathèque : les photos et vidéos perso.

## Étapes

Chaque étape a été livrée seule, testée, et vérifiée dans le navigateur sur ordinateur et sur téléphone.

1. **Le scan. Fait.** Une médiathèque de musique ne prend que ses fichiers audio. Chaque fichier est lu sur place par la crate `tags` (étiquettes, durée, format), et les rares formats qu'elle ne connaît pas (WMA, DSD) par l'outil d'analyse des films, en secours. Le rangement se décide un dossier à la fois (`library/src/music.rs`) : l'étiquette fait foi, les dossiers comblent ce qu'elle tait, un dossier `CD 2` est le second disque de l'album au-dessus, un album sans artiste d'album dont les morceaux sont joués par des gens différents est une compilation rangée sous « Various Artists ». La base écrit un dossier en une seule opération (`database/src/music.rs`) ; un morceau ré-étiqueté garde son identité, ses favoris et son historique, et les albums et artistes laissés vides sont retirés, jamais ceux d'un disque débranché.
2. **L'affichage. Fait.** Pochettes prises à côté des morceaux (`cover`, `folder`, `front`...) ou dans le premier morceau ; photo d'artiste dans le dossier au-dessus, sinon la pochette de son premier album. Onglets Pour vous, Albums, Artistes d'albums, Artistes, Chansons, Listes de lecture, Favoris et Genres ; tri, rail de lettres, chargement par pages de 200 ; pages d'album (disque par disque) et d'artiste. **L'accueil** a sa rangée « Ajouts récents dans Musique » et sa tuile dans la bande, pochettes en éventail. **La recherche** trouve artistes, albums et morceaux, dans le menu rapide comme sur la page, avec « Partout » ou limitée à la musique. Les écrans de la musique se relisent d'eux-mêmes quand leur médiathèque change (un scan, une pochette trouvée, des étiquettes écrites).
3. **Un lecteur simple. Fait.** Le son part tel quel quand le navigateur sait le lire, sinon converti à la volée en Opus ou en MP3 depuis le point demandé, sans session ni fichier gardé (`/api/v1/music/songs/{id}/sound`).
4. **Le lecteur complet. Fait.** Deux éléments audio qui se relaient dans `web/src/music/player/`, qui survivent à la navigation : le morceau suivant est préparé vingt secondes avant la fin et prend le relais sans blanc, ou en fondu enchaîné si le compte l'a choisi. Barre en bas sur toutes les pages, page en grand avec la file d'attente et les paroles, vrai bouton Arrêt, touches multimédia et écran verrouillé, file et position retrouvées après fermeture de l'onglet. Un film à l'écran arrête la musique, ou la met seulement en pause selon le réglage (signal neutre `web/src/on-screen.ts`).
5. **Les réglages Musique. Fait.** Une catégorie à part dans les réglages du compte, rangée dans ses propres tables : ce qu'un film fait à la musique, même volume pour tous (par morceau, par album ou désactivé), fondu enchaîné, file retrouvée ou non, débit maximum (au-delà, le morceau est converti), aperçu avant d'écrire des étiquettes.
6. **Favoris et écoutes. Faits.** Un cœur sur chaque morceau, album et artiste, et dans la barre ; une écoute compte à la moitié du morceau ou après quatre minutes ; « Écoutés récemment » et « Les plus écoutés » dans l'onglet Pour vous. Les favoris gardent la table de toutes les œuvres ; la musique a son propre magasin côté interface.
7. **Les listes de lecture. Faites.** À part de celles des films, dans leurs propres tables : un même morceau peut y être deux fois. Menu « ⋯ » sur chaque morceau (lire ensuite, ajouter à la file, ajouter à une liste, aller à l'album ou à l'artiste) et en tête d'album ; page d'une liste où l'on glisse pour réordonner, retire, renomme, supprime.
8. **Les paroles. Faites.** Cherchées de la plus proche à la plus lointaine : dans le morceau, dans un fichier `.lrc` du même nom à côté, puis sur LRCLIB si la médiathèque l'autorise (éteint par défaut, chaque morceau demandé une fois, réponse gardée). Les paroles minutées suivent la chanson, la ligne chantée est éclairée, et une ligne touchée y emmène.
9. **La normalisation du volume. Faite.** Le gain ReplayGain écrit dans le fichier est pris au scan ; les morceaux qui n'en ont pas sont mesurés (EBU R128) par un travail de fond qui suit chaque scan et reprend après un redémarrage. Le lecteur ramène chaque morceau (ou chaque album) à -14 LUFS par un gain du navigateur, sans jamais faire saturer le passage le plus fort.
10. **Le gestionnaire d'étiquettes. Fait.** Écran « Modifier les étiquettes » d'un album : album, artistes, année, genres, compilation, pochette, et pour chaque morceau disque, piste, titre et artistes. Renommage par modèle libre (`{track} - {title}`...), sans jamais déplacer de dossier ; copie des fichiers d'origine dans le dossier de données du serveur (`tag-backups`) quand la case est cochée ; aperçu de chaque changement avant d'écrire, sauf si le compte l'a désactivé. Il faut le droit « Modifier les étiquettes des morceaux » (les administrateurs l'ont) et que la médiathèque autorise l'écriture (éteint par défaut). Le scan qui suit relit les fichiers sous les mêmes morceaux : favoris, écoutes et listes sont gardés.
11. **Les pochettes en ligne. Faites.** Une médiathèque peut chercher les pochettes manquantes : MusicBrainz dit quel album c'est, le Cover Art Archive donne sa pochette. Éteint par défaut, une question par seconde comme MusicBrainz le demande, chaque album demandé une fois. Une pochette posée à côté des morceaux passe toujours devant.
12. **Les photos d'artiste en ligne. Faites.** Une médiathèque peut chercher sur Deezer les photos d'artiste manquantes, avec sa propre case, éteinte par défaut. Deezer a été retenu parce qu'il ne demande ni compte ni clé (fanart.tv et TheAudioDB en demandent une) ; il tolère cinquante questions en cinq secondes, Melyxar en pose au plus quatre par seconde. Seul un artiste de nom identique (majuscules et espaces autour mis à part) est pris, jamais un artiste sans photo chez Deezer, jamais « Various Artists ». Chaque artiste est demandé une fois ; cela se fait dans le même travail que les pochettes, en seconde étape. Une photo posée à côté des morceaux passe toujours devant.

13. **L'apparence reprend celle du reste de Melyxar. Fait.** Les pochettes, artistes et listes sont la carte de l'accueil (gros bouton lecture, halo au survol, cœur). Le lecteur, réduit comme agrandi, réutilise les classes, les icônes, la barre, les horloges et le réglage du son du lecteur vidéo (`player/sound.tsx` est commun aux deux), en sombre quel que soit le thème. « Écoutés récemment » et « Les plus écoutés » sont des rangées de tuiles larges, deux par colonne, comme chez Emby. Les dossiers que les systèmes gardent pour eux sur un disque (`$RECYCLE.BIN`, `System Volume Information`, `@eaDir`, `#recycle`, `lost+found`) ne sont jamais parcourus.
14. **Le spectre derrière le lecteur. Fait.** Le travail de fond qui mesurait le volume lit aussi le spectre de chaque morceau, dans la même lecture : 24 bandes, quatre fois par seconde. Une route de lecture seule le donne au navigateur, qui dessine une vague lisse derrière le lecteur réduit ; un réglage Musique du compte l'éteint (voir le README des décisions).

## Limites connues

- **Un album renommé par le gestionnaire d'étiquettes** devient un autre album une fois le scan passé : sa page d'origine dit alors qu'il n'est plus là, et on le retrouve dans la médiathèque sous son nouveau nom.
- **WMA** se lit et se joue, mais ses étiquettes ne s'écrivent pas.

## Décisions prises avec le mainteneur

- **Tous les formats audio courants se lisent**, y compris les plus rares (ALAC, WMA, APE, WavPack) : ce que le navigateur ne lit pas tel quel est converti en fichier audio simple par le chemin serveur de la musique.
- **Un lecteur en bas de l'écran, comme Emby et Jellyfin** : pochette, titre et artiste, précédent, lecture, arrêt, suivant, temps, volume, aléatoire, répétition, favori et file d'attente. Seule l'ergonomie est reprise, l'apparence est celle de Melyxar.
  - **Il reste toujours visible**, sur toutes les pages de Melyxar.
  - **La navigation ne l'interrompt jamais** : on va où on veut dans Melyxar, la musique continue sans coupure.
  - **On peut l'ouvrir en grand de n'importe où** : un clic sur le lecteur affiche la page « en cours de lecture » (pochette, paroles, file d'attente) dans Melyxar, pas en plein écran du navigateur ni de l'écran, et on la referme pour retrouver la page d'où l'on venait.
- **Page d'un album comme chez Jellyfin** : pochette, nom, artiste, nombre de pistes, boutons Lire, Aléatoire, favori et menu, puis la liste des morceaux avec leur durée, un favori et un menu par ligne.
- **Un vrai bouton Arrêt, distinct de la pause.** Arrêter met fin à la lecture et fait disparaître le lecteur, y compris la commande de lecture que le système affiche dans ses panneaux (téléphone, ordinateur), pour qu'on puisse la quitter. La pause, elle, laisse tout en place.
- **La réactivité est une exigence au même titre que pour le reste de Melyxar** (voir `02-reactivite.md`) : Emby est réactif, Jellyfin est lent, et Melyxar doit être au niveau du meilleur. Cela vaut pour le démarrage d'un morceau, le passage au suivant sans attente, le défilement d'une discothèque de plus de 100 000 morceaux, la barre de lettres, et le fait que le lecteur du bas ne ralentisse aucune page. Elle se mesure avec l'enregistreur intégré avant d'être gardée.
- **Lancer un film arrête la musique pour de bon par défaut**, réglable : chaque compte peut choisir de la mettre seulement en pause à la place. Le lecteur du bas disparaît pendant le film ; en mode pause, il revient à la fin du film avec la musique là où elle en était.
- **Une catégorie « Musique » dans les réglages du compte**, pour laisser le choix. Y vivent ce réglage, le mode de volume (même niveau pour tous ou écarts d'album gardés), les paroles et tout ce que la musique ajoutera. Le principe : chaque comportement discutable est un réglage, jamais imposé.
- **Comme Jellyfin sur tout ce qui touche au classement**, pour que ceux qui viennent de là retrouvent leurs repères : artistes d'albums séparés des artistes (les invités d'un morceau comptent parmi ces derniers), compilations sous « Artistes divers », albums en plusieurs disques groupés par disque, genres lus dans les étiquettes avec leur onglet, favoris (morceaux, albums, artistes), compteurs d'écoute pour « Écoutés récemment » et « Les plus écoutés ».
- **La photo d'un artiste** vient d'abord de son dossier (`folder.jpg`, `artist.jpg`), sinon, si la médiathèque le demande, de Deezer.
- **La file d'attente est retrouvée** quand on revient après avoir fermé l'onglet, avec le morceau et la position, et cette reprise se choisit ou non par réglage, comme pour les médiathèques.
- **Fondu enchaîné entre morceaux, en option** dans les réglages Musique.
- **Débit maximum de la musique, par compte** : au-delà, les morceaux sont convertis à la volée, utile pour écouter par un accès lent.
- **La recherche du haut trouve aussi la musique**, dans une section « Musique » de ses résultats, et cela marche quand la portée choisie est « Partout ».
- **Le volume laisse toujours le choix** : un même niveau pour tous les morceaux, ou les écarts d'un album gardés.
- **L'écoute en arrière-plan est prévue d'office** : écran verrouillé, onglet en arrière-plan, touches multimédia du clavier et du téléphone.
- **L'accueil a une rangée dédiée à la musique**, qui ne passe ni dans « Continuer la lecture » ni dans « À suivre ». Ses propres rangées (écoutés récemment, ajoutés récemment) vivent dans la médiathèque de musique.
- **Les paroles sont à mettre en place**, défilant en rythme quand elles portent l'heure de chaque ligne. Sources, de la plus locale à la plus distante : paroles intégrées au fichier, fichier `.lrc` à côté du morceau, puis LRCLIB (gratuit, sans clé) si le fournisseur est activé, réglable par médiathèque.
- **LRCLIB vérifié le 29 septembre 2026** : sans compte ni clé, une requête `GET /api/get` avec artiste, titre, album et durée rend les paroles seules et les paroles synchronisées (une heure par ligne, au format `[0:07.78]`), et une réponse 404 `TrackNotFound` quand le morceau est inconnu. La durée n'a pas besoin d'être exacte à la seconde. Le service répond parfois 503 « serveur occupé » : la recherche ne doit jamais bloquer ni la lecture ni le scan, elle réessaie plus tard. Aucune limite de débit ni condition d'usage n'est publiée : s'identifier par un `User-Agent`, ne demander qu'une fois par morceau et garder la réponse, y compris l'absence de réponse. **La question exacte ne suffit pas** : LRCLIB garde une entrée par album et par durée sous lesquels un morceau est sorti, et la demande exacte (artiste, titre, album, durée) répond « inconnu » pour un morceau qu'il connaît sous un autre album (constaté sur un morceau présent sous cinq albums et trois durées). Quand elle échoue, le morceau est cherché par artiste et titre : même titre et même artiste une fois l'orthographe mise de côté, l'entrée la plus proche en durée, ses paroles minutées données telles quelles seulement si elle est à moins de cinq secondes du fichier, sinon en texte simple. Les « inconnu » gardés avant cela ont été oubliés pour être redemandés. **Les étiquettes pauvres sont la règle, pas l'exception** : un fichier dont le titre est son nom de fichier (« Artiste x Autre - Titre - (Sous-titre) ») et dont l'artiste est son dossier ne trouve rien par artiste et titre, alors que LRCLIB le connaît. La recherche va donc de la plus étroite à la plus large : par artiste et titre, par le titre seul en texte libre (qui trouve un titre portant son artiste), puis par les deux en texte libre. Un résultat est le morceau demandé quand les **mots** de son artiste et de son titre, pris ensemble, ressemblent assez à ceux du fichier : sans accents ni majuscules, points d'une abréviation retirés, mots qui ne disent rien du morceau mis de côté (feat, prod, x, official, video, lyrics). Il faut 80 % de ressemblance, ou 50 % quand la durée est celle du fichier à trois secondes près. Le plus ressemblant gagne, une entrée avec paroles avant une entrée sans paroles, puis la durée la plus proche. Les réponses gardées comme « morceau sans paroles » ont été oubliées elles aussi, certaines venant d'un choix qui ignorait les entrées avec paroles.

- **LRCLIB, ses conditions écrites (documentation lue le 9 octobre 2026) et ce que Melyxar en fait.** Elle exige que le client **se nomme** par un `User-Agent` portant son nom, sa version et un lien ou une adresse : celui de Melyxar porte le lien du projet. Elle demande des questions **envoyées l'une après l'autre**, avec un repos de 200 à 500 millisecondes entre elles : toutes passent par une seule porte, qui attend 300 millisecondes entre deux. Elle impose de **respecter le `Retry-After` d'une réponse 429**, sous peine d'un bannissement temporaire : la porte refuse alors toute question pour la durée dite (soixante secondes quand elle ne la dit pas), et le morceau est redemandé plus tard sans rien garder. Son code est sous licence MIT et le service se dit gratuit et à but non lucratif : **elle est citée dans À propos**, avec un lien.
- **Choisir les paroles à la main. Fait.** Le menu d'un morceau propose « Paroles » à un administrateur : une fenêtre dit d'où viennent les paroles actuelles (fichier, fichier `.lrc`, LRCLIB), cherche sur LRCLIB par artiste et titre, modifiables, et laisse prendre une entrée, qui est gardée comme toute réponse de LRCLIB. Oublier des paroles venues de LRCLIB fait chercher le morceau de nouveau. Cette recherche à la main se fait quel que soit le réglage « chercher les paroles en ligne » de la médiathèque : ce réglage est pour ce qui est demandé sans que personne le demande. Les paroles à l'écran se relisent d'elles-mêmes.

## Corriger la musique : deux niveaux

- **Dans Melyxar, comme Jellyfin** : corriger un titre, un artiste, un album ou une pochette modifie seulement la fiche gardée par Melyxar, jamais le fichier. C'est le comportement de base, ouvert à ceux qui ont le droit de corriger.
- **Un gestionnaire d'étiquettes dédié**, en plus, qui modifie la fiche **et le fichier physique**, pour remplacer un logiciel de retouche d'étiquettes sous Windows. Il permet de :
  - corriger le titre, l'artiste, l'album et les autres étiquettes, et supprimer celles qui ne servent à rien ;
  - ajouter une image d'album, enregistrée sous le nom `cover.jpg` à la racine du dossier de l'album ;
  - renommer les fichiers à partir de leurs étiquettes.
- **Qui peut s'en servir** : les administrateurs, et les comptes à qui ce droit est donné.
- **Garde-fou** : une case à cocher, optionnelle, garde une copie de l'ancien fichier avant de l'écrire.
- **Aperçu avant d'écrire** : avant d'enregistrer, Melyxar montre ce qui va changer (étiquettes, nouveau nom de fichier) et on valide. C'est le comportement d'office, que chaque compte peut désactiver dans ses réglages Musique.
- **Renommage par modèle libre** : la forme du nom est écrite à partir des étiquettes (numéro de piste, titre, artiste, album, etc.).
- **Les fichiers ne sont jamais déplacés d'un dossier à un autre** : le gestionnaire renomme, il ne range pas dans des dossiers `Artiste/Album/`.
- **Écrire dans les fichiers reste éteint tant qu'on ne l'a pas demandé**, comme la règle déjà en place pour les médias (le service ne peut qu'y lire par défaut).
- **Isolation** : l'écriture des étiquettes vit dans son propre module, séparé de la lecture. Renommer un fichier ne doit rien faire perdre à la fiche (favoris, compteurs d'écoute, listes de lecture), sur le modèle du déplacement d'un film déjà géré.

## Vérifications techniques faites (29 septembre 2026)

- **Lecture des étiquettes** : l'analyse actuelle lance `ffprobe` fichier par fichier et sait déjà renvoyer les étiquettes du fichier (titre, artiste, album, piste), mais ne les expose pas. Lancer un `ffprobe` par morceau coûte environ 50 ms, soit plus d'une heure pour 100 000 morceaux sur un seul fil. **La bibliothèque Rust `lofty` lit les mêmes informations (étiquettes, durée) en quelques microsecondes par fichier, sans lancer de programme** : le scan de la musique la prendra pour lire, et gardera `ffprobe` seulement pour les formats qu'elle ne connaît pas.
- **Écriture des étiquettes** : `lofty` (licence MIT ou Apache 2.0, maintenue, très utilisée) a été essayée sur de vrais fichiers MP3, FLAC, M4A (AAC), OGG Vorbis, Opus, WavPack et ALAC : lecture, modification du titre et de l'artiste, suppression d'une étiquette, ajout d'une pochette intégrée, et **le son décodé est resté strictement identique dans les sept formats**. Relus ensuite par `ffprobe`, les nouvelles étiquettes sont bien là. Non couverts : WMA, qui reste en lecture seule (lue par `ffprobe`), et certains formats rares non essayés. Certaines informations techniques d'un MP3 (l'en-tête de l'encodeur) ne sont pas des étiquettes et ne se suppriment pas avec elles.
- **Écrire sans risque** : `lofty` modifie le fichier en place. Le gestionnaire écrira donc sur une copie de travail, vérifiera que le son est inchangé, puis remplacera le fichier, pour qu'une coupure en plein travail ne laisse jamais un fichier à moitié écrit.
- **Serveur existant** : le serveur accepte déjà de créer une médiathèque « musique », et rien n'est rangé sous cette sorte aujourd'hui. Son scan ne reconnaît que les fichiers vidéo : une médiathèque de musique créée maintenant resterait vide, et une vidéo posée dedans serait traitée comme un film. **La musique aura donc sa propre branche de scan avant que la création d'une médiathèque de musique soit ouverte à l'interface**, pour ne jamais passer par le chemin des films.

## Pistes possibles, sans priorité

Ouvertes à l'avenir, mais pas un objectif pour l'instant. Le chantier ne doit simplement pas les rendre impossibles.

- Clips vidéo rangés sous un artiste (Jellyfin le fait), lus par le lecteur vidéo.
- Livres audio, comme sorte de médiathèque à part, avec reprise à l'endroit où l'on s'est arrêté.
- Égaliseur dans le lecteur.
- Envoi de ce qu'on écoute à des services comme Last.fm ou ListenBrainz (le « scrobbling »), sans priorité mais pas une porte fermée : si des gens le veulent, on le fait.

## Questions ouvertes

À trancher avec le mainteneur avant de coder l'étape concernée. Les réponses sont ajoutées ici, puis au README des décisions.

Aucune pour le moment.
