# Fonctionnalités voulues

Ce document recense ce que le mainteneur veut que Melyxar sache faire, et ce que chaque demande implique dans l'architecture. Il se remplit au fil des discussions. Une fonctionnalité notée ici n'est pas forcément dans la V0.1 : la colonne « quand » dit à quel jalon elle arrive, et la section « ce que cela change » dit ce qu'il faut prévoir dès maintenant pour ne pas se bloquer.

## 1. Personnalisation et image du serveur

L'administrateur doit pouvoir remplacer facilement l'identité par défaut de Melyxar.

- Nom du serveur, logo, écran de démarrage, image de fond de la page de connexion.
- La page de connexion affiche ce nom et ce logo, avant toute authentification.
- Les fichiers envoyés par l'administrateur sont stockés côté serveur, pas dans le dépôt.

**Ce que cela implique.** Une table de paramètres du serveur et un répertoire de fichiers envoyés (dans les données, pas dans le cache, car ils ne sont pas régénérables). Surtout : une **route publique non authentifiée** qui renvoie l'identité visuelle, puisque la page de connexion doit l'afficher avant qu'un utilisateur existe. Cette route ne doit rien divulguer d'autre (ni la liste des utilisateurs, ni la version, ni les bibliothèques). Les fichiers envoyés sont validés (type réel, taille maximale) et servis depuis une route dédiée, jamais depuis un chemin donné par le client.

**Quand.** Jalon 3 pour la structure (paramètres, route publique), jalon 8 pour l'écran d'administration qui permet de les changer.

## 2. Interface et thème

- Interface **colorée**, jamais tout blanc ni tout noir.
- Une **couleur d'accentuation** que l'utilisateur choisit facilement.
- Modes **clair, sombre et automatique** (suivant le réglage du système d'exploitation).
- **CSS personnalisé** par l'utilisateur, dans l'esprit de ce que permettent Jellyfin et Emby, et aussi au niveau du serveur pour l'administrateur.
- Squelette de la page d'accueil et de la fiche de film **inspiré d'Emby**.
- Exigence forte : interface aboutie visuellement et très fluide.

**Ce que cela implique.**

- **Aucune couleur, aucun espacement, aucun rayon en dur dans les composants.** Tout passe par des jetons (variables CSS) définis en un seul endroit. C'est une règle de code dès le premier composant : ajouter un thème à une interface qui a des couleurs codées en dur oblige à repasser sur chaque fichier.
- La couleur d'accentuation choisie par l'utilisateur génère une petite palette dérivée (variantes claires et sombres, couleur de texte lisible par-dessus). Il faut vérifier le contraste automatiquement, sinon un utilisateur qui choisit un jaune vif se retrouve avec du texte illisible.
- Le mode automatique s'appuie sur la préférence système du navigateur, avec trois états distincts : clair forcé, sombre forcé, automatique. Le choix est stocké dans les préférences de l'utilisateur, pas seulement dans le navigateur, pour le retrouver sur un autre appareil.
- **Le CSS personnalisé est un point de sécurité.** Une feuille de style peut faire sortir des informations (une règle qui charge une image depuis un serveur externe en fonction de ce qui est affiché) et peut rendre l'interface inutilisable. Règles : seul un administrateur peut poser un CSS au niveau du serveur ; le CSS d'un utilisateur ne s'applique qu'à lui ; un paramètre permet de le désactiver ; et il existe toujours un moyen de revenir à l'interface normale sans CSS (une adresse de secours), sinon un CSS cassé bloque l'accès pour de bon.
- Sur le squelette inspiré d'Emby : reprendre une **disposition** (ce qui est placé où, les proportions, le parcours de l'utilisateur) est légitime. Reprendre du code ou des ressources graphiques d'Emby ne l'est pas, leur code est fermé et sous licence propriétaire. On s'inspire de la structure, on écrit notre propre code et nos propres visuels.

**Quand.** Jetons de thème et internationalisation dès le jalon 3, au premier composant. Choix de la couleur, modes clair et sombre, CSS personnalisé au jalon 8.

## 3. Navigation et suivi de lecture

- Système **lu / non lu**, marquage manuel possible.
- **Reprise de lecture** : retrouver exactement où on s'était arrêté.
- L'état est sauvegardé **en temps réel**, y compris si on quitte brutalement ou si la connexion est perdue.
- **Favoris**.
- **Compteurs d'épisodes** : nombre d'épisodes non vus par série et par saison.

**Ce que cela implique.**

- La progression est déjà prévue au modèle. Il faut y ajouter un état explicite (non commencé, en cours, vu) et une date de dernière lecture, plutôt que de déduire « vu » d'un pourcentage. Le seuil de passage automatique à « vu » (généralement autour de 90 % de la durée) est un paramètre, et un marquage manuel prime toujours sur l'automatique.
- **Les compteurs par série et par saison ne se comptent jamais à la lecture.** Une série de dix saisons demanderait un parcours de tous les épisodes de tous les utilisateurs à chaque affichage. Ce sont des valeurs stockées, mises à jour à chaque changement de progression : nombre d'épisodes, nombre non vus, date du plus récent épisode ajouté, pour chaque couple utilisateur et saison, et utilisateur et série. C'est exactement ce que décrit le document sur la réactivité : le calcul se fait à l'écriture.
- **La sauvegarde en temps réel qui survit à une coupure** demande trois mécanismes complémentaires :
  1. Le client envoie sa position régulièrement (toutes les dix secondes environ) et lors des événements importants (pause, seek, fin).
  2. Le client garde une copie locale de la dernière position non confirmée, et la renvoie à la reconnexion. Sans cela, fermer l'onglet entre deux envois perd jusqu'à dix secondes.
  3. Le serveur accepte une position accompagnée de son horodatage, et ne l'écrase jamais avec une plus ancienne. Sans cette règle, une position en retard arrivant après une plus récente ferait reculer la reprise.
- L'écriture de progression ne bloque jamais une réponse : elle est mise en file et confirmée immédiatement.

**Quand.** Progression et reprise au jalon 4. Favoris, lu/non lu manuel et compteurs au jalon 3 pour le modèle, visibles dans l'interface au même moment. Les compteurs de séries n'ont de sens qu'avec les séries, donc après la V0.1, mais **les colonnes et le mécanisme de mise à jour sont posés dès le départ**.

## 4. Lecture

### Indicateur de chargement avec progression

Quand une lecture démarre, l'utilisateur doit voir que quelque chose se passe, et pouvoir juger si ça avance ou si c'est bloqué.

**Ce que cela implique.** Le serveur expose l'état de préparation d'une session de lecture, et le client l'affiche.

Une précision honnête : un pourcentage unique et sincère n'existe pas, parce que la préparation n'est pas une tâche linéaire. Ce qui est à la fois vrai et utile, c'est une suite d'étapes nommées avec une progression réelle là où elle existe :

1. Analyse du fichier et décision de lecture (quasi instantané).
2. Démarrage du processus FFmpeg.
3. Production du premier segment. Ici une progression réelle est disponible, puisque FFmpeg rapporte où il en est par rapport à la durée du segment.
4. Mise en tampon dans le navigateur, dont la progression est connue précisément.

L'anneau de chargement affiche donc une progression réelle sur les étapes 3 et 4, avec le nom de l'étape en cours. Un blocage devient visible : l'étape ne change pas et le pourcentage n'avance plus. En cas d'échec, le message dit ce qui a échoué, pas « erreur inconnue ». Un faux pourcentage qui monte tout seul serait pire que rien, parce qu'il masquerait exactement le blocage qu'on veut voir.

**Quand.** Jalon 5, en même temps que les sessions de transcodage.

### Courbe de volume

Le réglage du volume doit répondre de façon régulière de 0 à 100 %, sans que tout se joue dans la moitié haute.

**Ce que cela implique.** L'erreur habituelle est de brancher directement la position du curseur sur la propriété de volume de l'élément vidéo, ou d'y appliquer une courbe trop agressive (cubique, ou basée sur les décibels). L'oreille ne perçoit pas l'amplitude linéairement : il faut une courbe de puissance modérée entre la position du curseur et l'amplitude appliquée, de l'ordre de 1,5 à 2, puis **vérifier à l'oreille sur toute la course**, notamment à 10, 25, 50 et 75 %. C'est un réglage court à écrire mais qui doit être testé, pas supposé. Le volume choisi est mémorisé dans les préférences de l'utilisateur.

**Quand.** Jalon 4, dès le premier lecteur.

### Bandes annonces

Depuis la fiche d'un film, pouvoir lancer la bande annonce.

**Ce que cela implique.** Deux sources, à gérer toutes les deux :

- **Bande annonce locale** : un fichier présent à côté du film (convention de nommage courante, par exemple un suffixe `-trailer`, ou un sous-dossier dédié). Elle se lit par le moteur de lecture normal, avec les mêmes règles de décision.
- **Bande annonce distante** : TMDb fournit des liens, presque toujours vers YouTube. La lire signifie intégrer un lecteur YouTube dans la page, donc une dépendance à un service externe et un pistage de l'utilisateur par ce service. C'est ce que font Jellyfin et Emby. Il faut un paramètre pour l'autoriser ou non, et l'utilisateur doit savoir qu'il sort du serveur local.

Le modèle doit donc accepter qu'une œuvre possède des **vidéos annexes** (bandes annonces, scènes coupées, making of), locales ou distantes. C'est une petite table à prévoir tôt, coûteuse à rajouter après.

**Quand.** Jalon 2 pour récupérer les liens, jalon 5 pour la lecture.

## 5. Bibliothèque musicale et normalisation audio

- Un type de bibliothèque dédié à la **musique**.
- **Normalisation audio** pour la musique comme pour les films.

**Ce que cela implique.** C'est la demande qui a le plus d'effet sur les fondations, et elle arrive au bon moment : avant la première migration.

- **Le modèle ne doit jamais supposer « un film ».** Une bibliothèque a un type (films, séries, musique). Les notions communes (source média, pistes techniques, progression, favoris, images) restent partagées ; les métadonnées propres à chaque domaine vivent dans leurs propres tables (film, série, saison, épisode, artiste, album, morceau). Si le schéma de la V0.1 met le titre du film et son année directement dans une table « œuvre » unique, ajouter la musique demandera une migration lourde. Si le schéma sépare dès le départ le tronc commun des métadonnées spécifiques, la musique devient un ajout et non une refonte.
- La musique apporte son propre fournisseur de métadonnées (MusicBrainz plutôt que TMDb), sa propre façon de naviguer (artiste, album, morceau, listes de lecture), et un volume d'éléments bien supérieur : une discothèque dépasse facilement les 100 000 morceaux. Les règles de réactivité valent donc plus encore.
- **La normalisation audio se mesure au scan, pas à la lecture.** La mesure de sonie selon la norme EBU R128 se fait par une analyse du fichier, qui prend du temps : c'est du chemin froid. Le résultat (sonie intégrée et crête) est stocké par piste audio, puis appliqué à la lecture sous forme de simple gain, ce qui ne coûte rien. Deux modes à prévoir : ramener chaque morceau au même niveau, ou conserver les écarts au sein d'un même album. Pour les films, le sujet voisin est la compression de plage dynamique lors du passage du multicanal à la stéréo, qui évite les dialogues inaudibles et les explosions assourdissantes.
- **Décision à prendre maintenant : ajouter la colonne de sonie aux pistes audio dès la première migration**, même si le calcul n'arrive qu'au jalon musique. Sinon il faudra réanalyser toute la bibliothèque.

**Quand.** Le modèle générique et les colonnes de sonie dès le jalon 0. La bibliothèque musicale complète est un jalon à part entière, après la V0.1. La normalisation pour les films peut arriver avec le transcodage.

## 6. Administration

### Tableau de bord

Un écran d'administration donnant le maximum d'informations sur le serveur en temps réel.

**Ce que cela implique.** Cet écran est l'extension naturelle de la page de diagnostic déjà prévue. Contenu utile : sessions de lecture en cours avec leur décision (lecture directe, remux, transcodage) et l'utilisateur concerné, charge processeur et mémoire, état de la carte graphique et sessions d'encodage, tâches de fond en cours et en attente, file d'écriture de la base, espace disque des trois répertoires, dernières erreurs, statistiques des bibliothèques.

Le temps réel passe par un flux d'événements du serveur vers le navigateur. Point d'attention : ce tableau de bord ne doit pas devenir lui-même une charge. Les mesures sont échantillonnées à intervalle fixe (une à deux secondes), et le flux s'arrête dès que personne ne regarde la page.

**Quand.** Jalon 7, en s'appuyant sur les mesures déjà posées aux jalons précédents.

### Mode maintenance

Quand l'administrateur doit intervenir, les utilisateurs doivent être prévenus proprement, et une page dédiée personnalisable doit s'afficher automatiquement.

**Ce que cela implique.**

- Un état du serveur, activable depuis l'administration, avec un message et une durée estimée saisis par l'administrateur.
- En maintenance, le serveur répond à toutes les requêtes ordinaires par une page dédiée accompagnée du code d'état HTTP prévu pour cela, tout en laissant passer l'administrateur. Ce code d'état est ce qui permet à un reverse proxy de réagir.
- Les lectures en cours sont averties plutôt que coupées brutalement, avec un compte à rebours avant l'arrêt effectif.
- Pour ceux qui ont un reverse proxy et veulent leur propre page : le serveur répondant déjà avec le bon code d'état, il suffit d'une directive nginx qui substitue une page locale. Nous fournirons un exemple de configuration. L'automatisme ne demande donc aucun dialogue entre Melyxar et nginx.

**Quand.** Jalon 8.

## 7. Accès et chiffrement

Trois possibilités proposées à l'administrateur, sans le noyer sous la technique :

1. **Derrière un reverse proxy** : Melyxar sert en HTTP simple, le proxy s'occupe du chiffrement. C'est la configuration du mainteneur et la plus courante.
2. **Certificat auto-signé généré par Melyxar** : le serveur fabrique son certificat et sert directement en HTTPS.
3. **Certificat fourni par l'administrateur** : il dépose ses fichiers, Melyxar les utilise.

**Ce que cela implique, et un avertissement.** Le serveur doit savoir servir en HTTPS lui-même, ce qui touche la crate serveur et la configuration. Ce n'est pas compliqué, mais il faut le prévoir dans la structure dès le départ plutôt que de brancher du TLS sur un serveur écrit pour HTTP seul.

Sur le certificat auto-signé, je dois être clair : **il ne permet pas d'éviter l'avertissement du navigateur.** Chaque appareil affichera un écran rouge de sécurité à la première visite, et il faudra cliquer pour passer outre, à moins d'installer l'autorité de certification sur chaque appareil, ce qui est précisément la manipulation technique qu'on voulait éviter. Le certificat auto-signé apporte le chiffrement, pas la tranquillité. Promettre le contraire dans l'interface serait mentir à l'administrateur.

La seule façon d'avoir du HTTPS vraiment sans friction est un certificat reconnu, ce qui suppose un nom de domaine. Avec un nom de domaine, même pointant vers une adresse locale, un certificat gratuit peut être obtenu et renouvelé automatiquement par validation DNS, sans exposer le serveur sur Internet. C'est donc une **quatrième option** à proposer, et c'est la seule qui tient la promesse « de bout en bout sans embêter l'utilisateur ». L'interface présentera les quatre choix avec, pour chacun, une phrase honnête sur ce qu'il donne et ce qu'il demande.

**Quand.** Jalon 9. La structure (configuration prévoyant HTTP ou HTTPS) dès le jalon 0.

## 8. Internationalisation

Toute chaîne affichée passe par le système de traduction. Dans le code, la clé est le mot anglais. Si une traduction manque, l'anglais s'affiche.

**Ce que cela implique.** Aucune chaîne en dur dans un composant, dès le premier écran. L'anglais est écrit en premier, le français ensuite. Cela vaut aussi pour les messages d'erreur renvoyés par le serveur : le serveur envoie un code d'erreur et des données, le client choisit le texte. Un serveur qui renvoie des phrases toutes faites ne peut pas être traduit par ses clients.

**Quand.** Dès le jalon 3, premier composant.

## 9. Clients natifs Android et Android TV

Le mainteneur utilise la télévision, pas le téléphone. La question est de savoir par où commencer.

**Recommandation : un seul projet Android, avec une base commune et deux interfaces, en commençant par celle de la télévision.**

- La base commune contient le client de l'API, la gestion de session, le cache et le lecteur vidéo. C'est la moitié du travail et elle est identique sur les deux.
- Les interfaces diffèrent complètement : sur une télévision, tout se navigue à la télécommande, avec une notion de focus et des éléments lisibles à trois mètres ; sur un téléphone, tout est tactile. Vouloir une seule interface pour les deux donne un résultat médiocre partout. Jellyfin maintient d'ailleurs deux applications séparées, ce qui lui coûte cher en doublons.
- Commencer par la télévision est le bon ordre pour deux raisons : c'est l'usage réel, et les contraintes de la télécommande sont les plus exigeantes. Une base pensée pour la télévision s'adapte bien au tactile ; l'inverse est douloureux.

**Ce que cela implique côté serveur.** Rien de neuf si les règles déjà posées sont respectées : authentification par jeton et non par cookie, images disponibles en plusieurs tailles, pagination partout, profil de capacités envoyé par le client, aucune logique métier côté client. Un client de télévision enverra simplement un profil différent. Le travail se fera bien plus tard, mais chaque écart à ces règles se paiera à ce moment-là.

**Quand.** Bien après la V0.1.

## 10. Ce que cela change dans les fondations

Résumé des décisions à intégrer **avant la première migration**, parce qu'elles sont coûteuses à rattraper.

| Décision | Pourquoi maintenant |
|---|---|
| Type de bibliothèque explicite, tronc commun séparé des métadonnées spécifiques au domaine | Sinon la musique impose une refonte du schéma. |
| Colonnes de sonie (EBU R128) sur les pistes audio | Sinon toute la bibliothèque devra être réanalysée. |
| Table des vidéos annexes (bandes annonces et autres) | Petite table, coûteuse à insérer plus tard dans des requêtes existantes. |
| État de progression explicite (non commencé, en cours, vu) et date de lecture | Déduire « vu » d'un pourcentage empêche le marquage manuel. |
| Compteurs agrégés par utilisateur et par saison ou série, mis à jour à l'écriture | Les compter à la lecture ruine la réactivité. |
| Favoris | Une table de plus, banale, mais qui doit exister avant que l'API ne se fige. |
| Table des préférences utilisateur (thème, couleur, langue, volume, CSS personnel) | Toutes ces fonctions y écrivent ; la créer après multiplie les migrations. |
| Table des paramètres du serveur et répertoire des fichiers envoyés | Le nom, le logo et les fonds de la page de connexion en dépendent. |
| Route publique d'identité visuelle, sans authentification et sans divulgation | La page de connexion doit afficher le logo avant toute connexion. |
| Configuration prévoyant HTTP ou HTTPS dès le départ | Brancher du TLS après coup sur un serveur écrit pour HTTP seul est une reprise en profondeur. |
| Erreurs renvoyées sous forme de code et de données, jamais de phrase toute faite | Sans cela, les clients ne peuvent pas traduire. |
| Aucune couleur ni espacement en dur dans le frontend, tout par jetons | Sinon le thème et le choix de couleur imposent de repasser sur chaque composant. |

## 11. Ce que cela change dans le plan

- **Jalon 0** gagne : paramètres du serveur, préférences utilisateur, répertoire des fichiers envoyés, structure HTTP ou HTTPS dans la configuration, et un schéma générique prêt pour la musique avec les colonnes de sonie.
- **Jalon 1** gagne : détection des bandes annonces locales pendant le scan.
- **Jalon 2** gagne : récupération des liens de bandes annonces depuis TMDb.
- **Jalon 3** gagne : jetons de thème et internationalisation dès le premier composant, disposition inspirée d'Emby, lu/non lu, favoris, route publique d'identité visuelle.
- **Jalon 4** gagne : courbe de volume vérifiée à l'oreille, sauvegarde de progression résistante aux coupures.
- **Jalon 5** gagne : état de préparation de lecture avec étapes nommées et progression réelle, lecture des bandes annonces.
- **Nouveau jalon 8** : personnalisation et administration (thème, couleur d'accentuation, CSS personnalisé, identité visuelle, page de connexion, mode maintenance).
- **Nouveau jalon 9** : accès et chiffrement, avec les quatre options.
- **Après la V0.1** : séries et compteurs visibles, bibliothèque musicale et normalisation, clients Android TV puis Android.
