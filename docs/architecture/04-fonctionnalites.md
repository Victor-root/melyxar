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

## 12. Utilisateurs, droits et confidentialité

Décisions validées.

- Plusieurs comptes, chacun avec sa progression, ses favoris et ses préférences.
- **Droits par utilisateur** : accès autorisé bibliothèque par bibliothèque, limite de classification d'âge, autorisation de télécharger, autorisation de supprimer, nombre de lectures simultanées.
- Connexion par mot de passe sur le web. **Code à quatre chiffres en option** pour les clients télévision, où taper un mot de passe à la télécommande est pénible. Le code est une commodité, pas une sécurité équivalente : il est propre à un appareil déjà autorisé une première fois par mot de passe, jamais un remplacement du mot de passe sur le web.
- **Journal d'activité** visible par l'administrateur : qui a regardé quoi et quand, connexions, actions d'administration. Effacement automatique après une durée configurable.
- **Détail des sessions en cours** : utilisateur, appareil et client, œuvre lue, position, décision de lecture et ses raisons, débit, en cas de transcodage la vitesse d'encodage et le matériel utilisé.

**Ce que cela implique.** Une table de droits par utilisateur et une table de journal d'activité. Le journal grossit vite : il est indexé par date, purgé automatiquement, et jamais lu sur le chemin chaud. Les sessions en cours vivent en mémoire et sont diffusées au tableau de bord par le flux d'événements, elles ne s'écrivent pas en base à chaque seconde.

**Quand.** Le modèle au jalon 0, l'application des droits au jalon 8, le journal et les sessions au jalon 7.

## 13. Personnes, collections, étiquettes et listes

Décisions validées.

- **Fiche complète** comme chez Emby : affiche, image de fond, titre, année, durée, classification, notes, synopsis, genres, réalisateur, acteurs avec photos, studios, bande annonce, versions disponibles, pistes audio et sous-titres, lecture ou reprise, favori, vu ou non vu, films similaires.
- **Acteurs et réalisateurs cliquables**, menant à leur filmographie dans la bibliothèque.
- **Sagas** : une section dédiée dans la navigation, listant les coffrets, chacun ouvrable pour voir ses films.
- **Étiquettes libres** et **listes de lecture personnelles**.

**Ce que cela implique.** Trois groupes de tables nouvelles, toutes à créer dès la première migration parce qu'elles s'insèrent dans des requêtes de navigation qu'il serait pénible de reprendre ensuite.

- **Personnes** : une table des personnes (nom, photo, identifiants externes) et une table de participation qui relie une personne à une œuvre avec son rôle (acteur, réalisateur, scénariste), le nom du personnage joué et l'ordre d'affichage. Une même personne ne doit exister qu'une fois, quelle que soit le nombre de films où elle apparaît, sinon la filmographie devient impossible.
- **Collections** : une table de collections et une table de liaison. Les collections viennent de deux sources : celles que TMDb connaît (une saga officielle) et celles créées à la main. Les deux cohabitent dans la même table avec une origine indiquée, sinon un rafraîchissement des métadonnées effacerait les collections manuelles.
- **Étiquettes et listes de lecture** : une table chacune plus une liaison. Les listes de lecture sont ordonnées et appartiennent à un utilisateur ; les étiquettes sont partagées.

**Quand.** Modèle au jalon 0, affichage au jalon 3.

## 14. Vignettes de chapitres et aperçu de la barre de lecture

Décision validée, avec un point technique important.

Le mainteneur veut des vignettes de chapitres, et surtout que **les vignettes tirées d'un film HDR ne soient pas délavées**.

**Le piège, et il est réel.** Extraire une image d'un film HDR sans traitement donne une image terne, grisâtre et désaturée. Ce n'est pas un défaut de qualité, c'est que les couleurs d'un fichier HDR sont codées pour un écran capable de les restituer ; lues telles quelles comme une image ordinaire, elles paraissent lavées. Beaucoup de serveurs multimédias ont ce défaut et leurs vignettes de films HDR sont laides. **La correction est la même opération que pour la lecture : convertir en SDR au moment de générer la vignette.** C'est donc une règle par défaut, pas une option : toute image extraite d'un fichier (vignette de chapitre, aperçu de la barre de lecture, aperçu au survol) passe par la conversion si la source est en HDR. Un réglage permettra malgré tout de désactiver la conversion pour ceux qui la veulent brute.

**Deux choses différentes**, à ne pas confondre :

- **Les vignettes de chapitres** : une image par chapitre, peu nombreuses, affichées dans la fiche ou dans un menu. Faible coût, faible encombrement.
- **L'aperçu de la barre de lecture** : la vignette qui apparaît quand on survole la barre de progression. Elle demande une image toutes les cinq ou dix secondes, soit des centaines par film. Stockées séparément, elles feraient des centaines de milliers de petits fichiers ; elles sont donc regroupées en planches (une grande image contenant une grille de vignettes), ce que font Plex, Emby et Jellyfin. Le coût en disque est réel et doit être annoncé : compter quelques mégaoctets par film, donc plusieurs gigaoctets pour une grande bibliothèque. L'intervalle et la résolution sont configurables, et la génération est activable par bibliothèque.

Dans les deux cas, la génération est une tâche de fond, avec priorité basse et accélération matérielle quand elle est disponible.

**Quand.** Vignettes de chapitres et aperçu de barre après le jalon 6, pour profiter de l'accélération matérielle. Les tables et les chemins de stockage sont prévus dès le jalon 0.

## 15. Confort de lecture

Décisions validées.

- **Reprise directe** à la position enregistrée, avec un bouton distinct pour repartir du début.
- **Mémoire des langues** : une préférence de langue par utilisateur pour l'audio et les sous-titres, plus une mémoire par série qui prime dessus.
- **Apparence des sous-titres réglable** : taille, couleur, contour, fond, position.
- **Enchaînement automatique de l'épisode suivant**.
- **Saut d'intro et de générique**, signalé comme très important.
- **Vitesse de lecture**, **image dans l'image** signalée comme très importante, **raccourcis clavier**.

**Ce que cela implique.**

- L'image dans l'image et la vitesse de lecture sont des fonctions natives du navigateur, disponibles aussi bien en lecture directe qu'en flux HLS. Peu de travail, à condition que le lecteur soit écrit comme un module isolé possédant l'élément vidéo, ce qui est déjà la règle.
- **Le saut d'intro demande un vrai travail.** Aucune métadonnée publique ne dit où commence un générique. Trois sources possibles, à combiner : les chapitres présents dans le fichier quand ils existent et portent un nom explicite ; une détection automatique qui compare les empreintes sonores des épisodes d'une même saison pour trouver le passage commun, ce qui est la méthode utilisée par l'extension correspondante de Jellyfin ; et la correction manuelle. C'est une analyse de fond coûteuse, à l'échelle d'une saison entière, mais elle ne se fait qu'une fois.
- Le modèle doit donc prévoir des **segments repérés** dans une source média : début et fin, avec un type (récapitulatif, générique de début, générique de fin, publicité). Le même mécanisme sert au bouton « épisode suivant » qui apparaît pendant le générique de fin. Cette table est à créer dès le départ même si elle ne se remplit qu'avec les séries.

**Quand.** Reprise, langues, vitesse, image dans l'image et raccourcis au jalon 4. Apparence des sous-titres au jalon 5. Enchaînement et saut d'intro avec les séries, après la V0.1 ; la table des segments dès le jalon 0.

## 16. Correction et gestion des médias

Décisions validées.

- **Correction d'identification depuis l'interface** : rechercher le bon film chez le fournisseur, voir les propositions avec affiche et année, en choisir une, et la fiche se met à jour. Le mainteneur souligne que cette fonction marche bien chez Emby et Jellyfin et qu'il faut s'en inspirer.
- **Modification manuelle d'une fiche** avec verrouillage des champs modifiés, qu'un rafraîchissement n'écrase jamais.
- **Suppression depuis l'interface**, avec une **case à cocher pour supprimer aussi le fichier du disque**, cette case étant réservée à l'administrateur.
- **Téléchargement d'un fichier** pour un visionnage hors ligne, soumis à une autorisation par utilisateur.

**Ce que cela implique.**

- La recherche manuelle demande une route qui interroge le fournisseur avec un texte libre et une année facultative, et une route qui applique la correspondance choisie. Elle doit aussi permettre de saisir directement un identifiant TMDb ou IMDb, ce qui est souvent le plus rapide quand le titre est ambigu.
- **La suppression du fichier sur le disque est l'opération la plus dangereuse de tout le projet.** Règles sans exception : jamais de chemin venant du client, seulement un identifiant interne que le serveur résout lui-même ; vérification que le chemin résolu est bien sous une racine déclarée ; case décochée par défaut et texte explicite disant ce qui sera effacé ; réservée à un administrateur disposant du droit correspondant ; et une entrée dans le journal d'activité. Une suppression seulement en base, sans toucher au disque, reste l'option par défaut.
- **Point pratique sur les permissions du LXC.** Vos dossiers médias appartiennent à l'utilisateur et au groupe `jellyfin` avec les droits d'écriture pour le groupe. Pour que Melyxar puisse effacer un fichier, son utilisateur système devra appartenir à ce groupe. Tant que ce n'est pas fait, la lecture fonctionnera parfaitement mais la suppression sur disque échouera. À traiter au moment où la fonction sera activée, pas avant.
- Le téléchargement sert le fichier d'origine avec le droit correspondant vérifié. Il ne doit pas passer par le chemin de transcodage.

**Quand.** Correction d'identification au jalon 2. Modification manuelle au jalon 3. Suppression et téléchargement au jalon 8, avec les droits.

## 17. Sauvegarde

Décision validée : sauvegarde automatique de la base, quotidienne, avec quelques copies conservées. La base étant un simple fichier, la sauvegarde se fait avec le mécanisme prévu par SQLite, qui produit une copie cohérente pendant que le serveur tourne. Les fichiers envoyés par l'administrateur (logo, fonds) sont inclus. Le cache d'images et les transcodages ne le sont pas, ils se régénèrent.

**Quand.** Jalon 8.

## 18. Hors périmètre

Exclus définitivement : télévision en direct, enregistrement, extensions tierces, gestion de téléchargements automatiques.

**Trakt** est écarté pour l'instant, sans conséquence sur les fondations : la synchronisation se brancherait plus tard sur les événements de progression déjà prévus. Le paragraphe ci-dessous décrit ce dont il s'agit, pour mémoire.

**Trakt, pour mémoire.** C'est un service en ligne qui tient l'historique de tout ce qu'une personne regarde, quelle que soit l'application utilisée. Marquer un film comme vu dans Melyxar le ferait apparaître sur le profil Trakt, et une autre application saurait qu'il est vu. Cela sert à ceux qui utilisent plusieurs applications, qui veulent des statistiques annuelles ou qui suivent des listes publiques. En contrepartie, cela demande un compte chez un tiers et cela envoie l'historique de visionnage à ce tiers. Ce n'est pas structurant : la synchronisation se brancherait plus tard sur les événements de progression déjà prévus. Décision reportée, sans conséquence sur les fondations.

## 19. Métadonnées, recherche et navigation

Décisions validées.

- **Langue des métadonnées** : français en priorité, repli sur l'anglais quand le français manque. Les deux versions sont stockées quand le fournisseur les donne, ce qui permet de basculer l'affichage d'une fiche sans requête externe.
- **Écran de choix d'utilisateur** avec avatars, avant la saisie du mot de passe, avec un réglage pour le désactiver et revenir à une saisie du nom.
- **Sous-titres manquants** : recherche et téléchargement depuis un service en ligne, **sur demande explicite uniquement**, jamais automatiquement. La qualité des sous-titres publics est inégale et un téléchargement de masse remplirait la bibliothèque de fichiers douteux.
- **Recherche globale** : titres, personnes, collections, avec résultats groupés par type.
- **Tri** par titre, date d'ajout, année, note, durée. **Filtres** par genre, décennie, non vu, favoris, résolution, présence de sous-titres.
- **Liste « à voir plus tard »**, distincte des favoris.
- **Statistiques personnelles** : temps de visionnage, films vus sur une période, genres préférés.

**Ce que cela implique.** Les métadonnées textuelles sont stockées par langue, ce qui veut dire une table de traductions plutôt que des colonnes de texte sur l'œuvre. C'est une décision de schéma à prendre maintenant. L'écran de choix d'utilisateur passe par la route publique déjà prévue, qui renverra alors la liste des comptes et leurs avatars : c'est une divulgation volontaire, d'où le réglage pour la désactiver. Les statistiques se calculent depuis le journal d'activité, sans table supplémentaire, mais sur des données historiques qui doivent survivre à la purge du journal : un résumé mensuel agrégé est conservé au-delà.

**Quand.** Langue et traductions au jalon 2. Recherche, tri, filtres et liste à voir au jalon 3. Choix d'utilisateur et statistiques au jalon 8. Sous-titres en ligne après la V0.1.

## 20. Plusieurs versions d'une même œuvre

Décision révisée : **un sélecteur de version est bien voulu.** Le mainteneur aura plusieurs copies du même film pour ses tests (conteneurs, codecs et pistes audio différents), et doit pouvoir passer de l'une à l'autre.

**Ce que cela implique.**

- La fiche affiche un sélecteur listant chaque version avec ce qui la distingue : résolution, codec vidéo, pistes audio, taille du fichier, conteneur. Un simple nom de fichier ne suffit pas à choisir, c'est la description technique qui compte, et elle est déjà en base grâce à l'analyse des pistes.
- Une version est proposée par défaut, choisie sur des critères simples et prévisibles : la résolution la plus haute, puis le débit le plus élevé. Pas de choix dépendant de la connexion ou de l'appareil, qui rendrait le comportement imprévisible pendant les tests.
- Le choix d'une version est mémorisé pour la session en cours, pas de façon permanente : relancer le film plus tard repart sur la version par défaut.
- La progression reste attachée à l'œuvre, pas à la version. Regarder la moitié d'un film puis reprendre sur une autre copie reprend au bon endroit.
- Le sélecteur n'apparaît que lorsqu'il y a plus d'une version, ce qui est le cas ordinaire.

**Quand.** Modèle au jalon 0, déjà présent. Sélecteur et choix par défaut au jalon 3 pour l'affichage, actif à la lecture au jalon 4.

## 21. Contrôle à distance

Piloter la lecture d'un appareil depuis un autre est souhaité mais explicitement **sans priorité**, à traiter bien plus tard. L'envoi vers un Chromecast n'est pas décidé.

**Ce que cela implique dès maintenant.** Rien, à une condition déjà respectée : les sessions de lecture sont identifiées côté serveur et diffusées par le flux d'événements. Un pilotage à distance consiste alors à envoyer une commande à une session existante, ce que la structure permettra sans refonte.

## 22. Installation, mise à jour et publication

### Script d'installation

L'installation et la première configuration dans le LXC se font par un **script shell unique**, reprenant exactement les conventions des scripts du dépôt `Proxmox-Tools` du mainteneur, qui sont sa signature. Les conventions relevées, à respecter à la lettre :

- En-tête `#!/usr/bin/env bash`, puis `set -Eeuo pipefail`, et `umask 077` quand le script écrit des fichiers sensibles.
- Sections séparées par des commentaires encadrés de caractères de filet, par exemple `# ── Couleurs ────`.
- **Bilingue, intégré au script.** La langue est déduite de la locale du système, l'anglais étant le repli pour une locale inconnue comme pour une clé manquante. Les textes vivent dans un tableau associatif chargé depuis un bloc de données au format `langue|clé|texte`, avec deux fonctions d'accès, l'une pour un texte simple, l'autre pour un texte contenant des valeurs à insérer.
- **Couleurs sur 256 niveaux**, désactivées si la sortie n'est pas un terminal ou si la variable d'environnement correspondante est posée. Les rôles sont toujours les mêmes : couleur principale, sa variante foncée, sa variante douce, ambre pour les avertissements, vert pour les succès, cyan, bleu, gris, atténué, gras, remise à zéro.
- **La couleur principale suit le sujet du script** : orange Proxmox pour les outils Proxmox, rouge pour WireGuard. Melyxar aura donc la sienne, cohérente avec la couleur d'accentuation par défaut de l'interface web.
- Largeur de terminal lue dynamiquement et plafonnée à 92 colonnes, trait de séparation dessiné à cette largeur.
- Messages courts préfixés d'un symbole : coche pour un succès, chevron pour une information, triangle pour un avertissement, croix pour une erreur, avec une fonction qui affiche l'erreur puis s'arrête.
- Encadrés d'explication dessinés avec des filets d'angle, pour les passages où l'utilisateur doit comprendre un choix.
- Bannière en lettres capitales dessinées en caractères de bloc, suivie du nom de l'outil et de la mention de l'auteur, puis d'un trait.
- **Exécution des étapes longues avec un indicateur animé**, la sortie de la commande étant capturée dans un fichier temporaire : en cas de succès la ligne devient une coche, en cas d'échec une croix suivie de la sortie complète, indentée.
- Invites de saisie préfixées d'un point d'interrogation, avec la valeur par défaut affichée entre crochets, et des confirmations acceptant les formes française et anglaise du oui et du non.
- Menu interactif numéroté pour les actions.
- Sauvegarde avant toute modification, et restauration possible.
- Aucune dépendance à installer au-delà de bash.

Ce que le script fera pour Melyxar : vérifier le système, installer les outils nécessaires, créer l'utilisateur système et les répertoires, récupérer et compiler le projet, écrire la configuration, installer le service, proposer le mode d'accès, et afficher l'adresse à ouvrir. Il servira aussi aux mises à jour et à la désinstallation propre.

### Publication et notification de mise à jour

- Chaque version publiée correspond à une **étiquette de version sur GitHub**.
- Le serveur vérifie périodiquement s'il existe une version plus récente et l'indique dans l'administration. **Aucune mise à jour automatique** : c'est l'administrateur qui lance la mise à jour.
- Cette vérification est un appel sortant vers GitHub : elle est désactivable, et son échec ne perturbe rien.

**Quand.** Script d'installation au jalon 0, dans sa forme minimale, complété au fil des jalons. Notification de version au jalon 8.

## 23. Navigation au clavier

Le mainteneur la veut si elle est simple, sans en faire une priorité.

**Distinction utile.** Il y a deux niveaux, très différents en coût.

- **Le niveau gratuit, à faire dès le début** : ne pas casser le comportement natif du navigateur. Utiliser de vrais boutons et de vrais liens plutôt que des éléments décoratifs rendus cliquables, respecter l'ordre de tabulation, afficher un contour visible sur l'élément sélectionné. Cela ne coûte presque rien pendant l'écriture et c'est très coûteux à rattraper, parce qu'il faut alors reprendre chaque composant. C'est aussi ce qui rend l'interface utilisable par les outils d'accessibilité.
- **Le niveau exigeant, retenu lui aussi** : la navigation directionnelle, où les flèches déplacent la sélection de vignette en vignette dans une grille, comme sur une télévision. Cela demande un gestionnaire de focus qui connaît la position de chaque élément à l'écran. Le mainteneur a choisi de le faire dès le départ. C'est le bon arbitrage à une condition : le gestionnaire est écrit **en même temps que le premier composant de grille**, pas plus tard. Greffé après coup sur des grilles existantes, il coûte plusieurs fois plus cher. Son autre avantage est de préparer directement le client télévision, qui repose sur la même logique.

**Quand.** Niveau gratuit et gestionnaire de focus directionnel au jalon 3, avec la première grille.

## 24. Identité visuelle de Melyxar

**Couleur principale : `#c81e1e`**, un rouge franc choisi par le mainteneur. Elle sert à la fois de couleur de thème du script d'installation et de couleur d'accentuation par défaut de l'interface web.

**Usage exact, précisé par le mainteneur.** Cette couleur est employée **en remplissage**, pas en couleur de texte : fond du bouton de lecture principal, barre de défilement, éléments actifs. Le texte posé dessus est blanc. Elle garde la même valeur en thème clair comme en thème sombre.

**Ce que la mesure confirme.** Du blanc sur ce rouge atteint un contraste d'environ 5,7 pour 1, au-dessus du seuil de lisibilité : un bouton rempli de cette couleur avec un libellé blanc est correct sur n'importe quel fond. Du noir dessus ne monterait qu'à 3,7, donc le texte posé sur cette couleur est toujours blanc, jamais noir. En revanche, ce rouge employé comme **couleur de texte fin sur fond sombre** ne donne qu'environ 3,2 pour 1 : insuffisant pour du texte courant, suffisant pour un élément d'interface non textuel comme une barre de défilement. Règle retenue : la couleur telle quelle pour les remplissages et les éléments graphiques dans les deux thèmes, et une variante éclaircie de la même teinte réservée aux liens et aux petits textes colorés sur fond sombre. C'est le rôle de la palette dérivée décrite plus haut.

Pour le script d'installation, le rouge tient le rôle de couleur principale et ses variantes foncée et douce s'en déduisent, sur le modèle du script WireGuard du dépôt `Proxmox-Tools`.

## 25. Interface web sur téléphone

Décision validée : l'interface web doit être utilisable depuis le navigateur d'un téléphone, en attendant les applications natives.

**Ce que cela implique.** Peu de chose si c'est prévu dès le premier écran, beaucoup si cela arrive après. Concrètement : grilles qui se réorganisent selon la largeur, zones tactiles suffisamment grandes, lecteur vidéo utilisable au doigt avec les gestes attendus, pas de survol comme seul moyen d'accéder à une action, et menus adaptés à un écran étroit. Le préchargement au survol décrit plus haut n'existe pas sur un écran tactile : l'équivalent y est le préchargement de la fiche dès que la vignette devient visible.

## 26. Assistant de première configuration

Le mainteneur demandait ce que recouvre ce terme. Il s'agit de ce que font Jellyfin et Emby au premier démarrage : plutôt que d'ouvrir une interface vide où l'on ne sait pas quoi faire, le serveur présente une courte suite d'écrans guidés.

Pour Melyxar, ce serait, dans l'ordre :

1. Choix de la langue de l'interface.
2. Création du compte administrateur, avec son nom et son mot de passe.
3. Ajout des bibliothèques : donner un nom, choisir le type (films, séries, animés, musique), et désigner les dossiers en parcourant l'arborescence du serveur plutôt qu'en tapant un chemin. Plusieurs dossiers par bibliothèque, ce qui correspond aux quatre disques du mainteneur.
4. Langue préférée des métadonnées et clé du fournisseur.
5. Mode d'accès, si le script d'installation ne l'a pas déjà réglé.
6. Écran final proposant de lancer le premier scan.

**Déclenchement.** L'assistant s'ouvre automatiquement tant que la configuration initiale n'est pas terminée, quelle que soit l'adresse demandée, et se marque comme achevé une fois parcouru. Une partie de ces étapes peut déjà avoir été remplie par le script d'installation : l'assistant saute alors ce qui est connu plutôt que de le redemander.

**Quand.** Jalon 8, sauf la création du compte administrateur qui existe dès le jalon 0 sous une forme minimale.

## 27. Nouveautés

Décision validée : pas de notification poussée ni de courriel. Les nouveautés se voient dans une **section « Récemment ajoutés »** sur la page d'accueil, à la manière d'Emby, alimentée par la date d'ajout déjà stockée et indexée.

## 28. Analyse des noms de fichiers

Le mainteneur a fourni un échantillon de sa bibliothèque réelle, **à ne pas reproduire dans le dépôt**. Seules les formes observées sont consignées ici, illustrées par des exemples inventés. Les jeux de tests utiliseront eux aussi des titres inventés couvrant ces mêmes formes.

### Structure observée

- **Fichiers posés à plat** dans le dossier de la bibliothèque, sans dossier par film. Le nom du fichier est donc la seule source d'information : rien ne peut être déduit d'un dossier parent. L'organisation en un dossier par film, courante ailleurs, doit malgré tout être reconnue, mais elle n'est pas la forme principale ici.
- Le **point sert de séparateur de mots** sur toute la longueur du nom.
- Le **titre précède l'année**, et l'année sur quatre chiffres est la frontière fiable entre le titre et le reste.
- Après l'année vient une **suite d'étiquettes techniques** de longueur variable : langues et pistes, définition, source, codec vidéo, codec audio, nombre de canaux.
- Un **groupe de diffusion** peut terminer le nom après un tiret.

### Règles à retenir

1. **Découper sur la dernière année plausible.** Prendre le dernier groupe de quatre chiffres compris dans une plage raisonnable, à condition qu'il soit suivi d'étiquettes techniques et non de mots du titre. Cette règle traite correctement les titres qui contiennent eux-mêmes une année.
2. **Ne jamais se fier au seul titre pour identifier un film.** L'échantillon contient deux films dont le titre commence identiquement mais dont l'année diffère. L'année est donc indispensable à la recherche chez le fournisseur.
3. **Le soulignement est un séparateur au même titre que le point.** Des marqueurs personnels apparaissent collés à la dernière étiquette technique sans séparateur propre. Sans cette règle, l'étiquette est méconnue et peut se retrouver dans le titre.
4. **Tout ce qui suit l'année est jeté du titre**, connu ou non. Le titre se construit exclusivement à partir de ce qui précède l'année, ce qui évite d'avoir à connaître toutes les étiquettes existantes.
5. **Ne pas confondre un mot court du titre avec une étiquette.** Certains titres contiennent des mots d'une seule lettre qui ressemblent à des marqueurs. Puisque la règle précédente ne filtre que ce qui suit l'année, ce piège disparaît, mais il ne faut pas le réintroduire en filtrant des mots à l'intérieur du titre.
6. **La casse est incohérente** d'un fichier à l'autre pour un même titre. Le titre extrait est donc normalisé avant la recherche, et la comparaison ignore la casse et les accents.
7. **Les étiquettes reconnues sont exploitées, pas seulement ignorées** : elles renseignent la définition annoncée, la source, les langues présentes et la présence de sous-titres. Elles restent indicatives : **la vérité technique vient de l'analyse du fichier**, jamais du nom. Un fichier annoncé en une définition et encodé dans une autre n'est pas un cas rare.

### Tests

Un jeu de cas écrit avec des titres inventés couvre chacune de ces formes : titre simple, titre à plusieurs mots, titre contenant un mot d'une lettre, titre contenant une année, marqueur personnel collé, groupe de diffusion terminal, casse incohérente, deux films au titre proche distingués par l'année. Chaque cas indique le titre et l'année attendus. Ce jeu est enrichi à chaque anomalie constatée sur la bibliothèque réelle, toujours sous forme de cas inventé équivalent.

## 29. Disposition de la fiche, relevée sur Emby

Le mainteneur a fourni des captures de la fiche d'un film chez Emby comme référence de direction. Melyxar reprendra cet agencement, avec des écarts assumés plus tard. On reprend la structure et le parcours, jamais le code ni les visuels, qui sont propriétaires. Les captures montrent des titres et des chemins réels : rien de tout cela n'est reproduit ici.

### Bandeau supérieur

Retour, accueil et menu à gauche, puis l'image de titre du film en petit format. À droite : diffusion vers un appareil, recherche, avatar du compte, réglages. Le bandeau reste visible au défilement et l'image de titre y rappelle où l'on se trouve.

### Bloc principal

Le fond de page est l'image de fond du film, assombrie. Par-dessus :

- **Affiche** à gauche, en grand format.
- **Titre** en très gros caractères, suivi immédiatement de deux icônes discrètes : modifier la fiche, et gérer les images. C'est le point d'entrée de la correction manuelle, placé là où l'on regarde déjà.
- **Ligne de métadonnées compacte** : note du fournisseur, note d'un agrégateur, année, durée, classification d'âge du pays, genre principal, et **heure de fin estimée si la lecture démarrait maintenant**.
- **Ligne technique et sélecteurs** : la définition résumée en quelques mots (définition, plage dynamique, codec), puis une liste déroulante de piste audio et une liste déroulante de sous-titres, **disponibles avant le lancement**. Les sous-titres peuvent être désactivés depuis cette liste.
- **Rangée d'actions** : bouton de lecture bien visible, bouton de bande annonce à côté, puis des boutons ronds à icône pour marquer vu, mettre en favori, supprimer, et un menu de trois points. Le bouton « vu » est rempli en couleur d'accentuation quand l'état est actif : l'icône porte l'état, il n'y a pas de texte à lire.
- **Accroche** en gras, distincte du synopsis.
- **Synopsis**, avec une règle qui s'écarte d'Emby, voir ci-dessous.
- **Réalisateur** nommé juste en dessous.

**Écart assumé sur le synopsis.** Emby coupe le texte à une hauteur fixe et affiche un lien pour dérouler, même quand le synopsis est plus court que la place disponible. Résultat visible sur la capture : un grand vide entre le synopsis et la section suivante. C'est le défaut à ne pas reproduire. La règle pour Melyxar : **le bloc occupe la place disponible et le texte la remplit ; le lien pour dérouler n'apparaît que si le texte déborde réellement.** Un synopsis court se lit donc en entier, sans bouton et sans vide. La place disponible se calcule à partir de la hauteur du bloc de gauche, l'affiche servant de référence, pour que les deux colonnes se terminent ensemble. Aucune hauteur fixe codée en dur : la règle vaut aussi bien sur un écran large que sur un téléphone, où la place disponible est différente.

### Sections suivantes, en rangées horizontales

Chacune est un titre de section suivi d'une bande défilante horizontalement.

- **Distribution et équipe** : photos des personnes en vignettes.
- **Chapitres** : vignette, libellé du chapitre, horodatage. Dans la capture fournie, les chapitres sont régulièrement espacés, ce qui correspond à des chapitres générés plutôt qu'à des chapitres nommés présents dans le fichier. Melyxar affichera les deux, en privilégiant les chapitres réels quand le fichier en contient.
- **Similaire** : affiches avec titre et année, et **une pastille de coche en coin pour les titres déjà vus**. Cet état sur la carte elle-même est à reprendre partout, y compris dans les grilles de bibliothèque.

### Section « À propos »

Un bloc d'informations détaillées, en bas de page :

- Date de dernière lecture, genres, studios, liens vers les fiches externes.
- **Informations du média** : chemin du fichier, conteneur, taille, date d'ajout.
- Puis **une carte par piste**, côte à côte, avec une icône par type : une carte vidéo, une carte par piste audio, une carte par piste de sous-titres.

**Une différence volontaire avec Emby sur ce point.** Emby affiche le chemin complet du fichier à tout utilisateur. Pour Melyxar, le chemin et la taille ne seront visibles que par un administrateur : ce sont des informations sur l'organisation du serveur, pas sur le film. Le reste du bloc technique reste visible par tous.

### Champs techniques à stocker et à afficher

Les cartes de la capture donnent la liste exacte de ce qu'un utilisateur exigeant veut voir. Elle devient la cible de l'analyse du fichier au jalon 1, et elle recoupe ce que l'outil d'analyse fournit déjà.

- **Piste vidéo** : titre lisible, codec, profil, niveau, résolution, ratio d'aspect, entrelacement, images par seconde, débit, plage dynamique, couleurs primaires, espace colorimétrique, courbe de transfert, profondeur des échantillons, format des pixels, nombre d'images de référence.
- **Piste audio** : titre lisible, langue, codec, disposition des canaux, nombre de canaux, taux d'échantillonnage, profondeur des échantillons, débit, piste par défaut.
- **Piste de sous-titres** : titre lisible, langue, codec, par défaut, forcée, destinée aux malentendants, interne ou externe.

Les champs de couleur (primaires, espace, courbe de transfert, profondeur) ne sont pas là pour décorer : **ce sont eux qui déterminent la décision de conversion en SDR**, pour la lecture comme pour les vignettes. Les stocker dès le premier scan est donc une nécessité fonctionnelle, pas un luxe d'affichage.

### Confirmation du problème des vignettes HDR

La capture des chapitres porte sur un film en 4K avec plage dynamique étendue. **Ses vignettes y apparaissent très sombres, la première presque entièrement noire.** C'est exactement le défaut décrit plus haut : des images extraites d'une source à plage dynamique étendue sans conversion. Le point n'est donc pas théorique, il est visible sur la référence elle-même. La règle retenue pour Melyxar, convertir systématiquement à l'extraction, est confirmée par ce cas réel.

## 30. Emploi de la couleur d'accentuation

Le mainteneur juge qu'Emby **sous-emploie** sa couleur d'accentuation : l'interface reste trop grise et la couleur ne sert presque qu'au bouton de lecture. Melyxar doit en mettre davantage.

**La règle, pour que « davantage » ne devienne pas « partout ».** La couleur garde une signification : elle marque ce qui est actif, ce qui est sélectionné, ce qui progresse et ce sur quoi on peut agir en premier. Étalée sur des éléments neutres, elle cesse de signaler quoi que ce soit et l'interface devient fatigante. On en met donc nettement plus qu'Emby, mais toujours sur des éléments porteurs de sens.

**Où la couleur est présente :**

- Bouton d'action principal, rempli.
- Élément de navigation en cours, dans le menu comme dans les onglets.
- **Contour de l'élément sélectionné au clavier ou à la télécommande.** C'est le repère le plus important de la navigation directionnelle, il doit être franc et immédiatement repérable.
- Barre de progression de lecture, et barre de progression sur les cartes des titres commencés.
- Barre de défilement.
- États actifs des boutons à bascule : vu, favori, à voir plus tard.
- Pastilles d'état sur les cartes, dont la coche de vu et le repère des ajouts récents.
- Ligne ou point de section devant les titres de rangées, en marqueur discret.
- Élément retenu dans une liste déroulante, y compris les sélecteurs de version, de piste audio et de sous-titres.
- Anneau de chargement au démarrage d'une lecture.
- Liens et textes interactifs, avec la variante éclaircie sur fond sombre.

**Où elle reste absente :** le texte courant, les grandes surfaces de fond, les bordures ordinaires des cartes, et les libellés d'information. Ces éléments restent neutres, et c'est ce qui donne sa force à la couleur partout ailleurs.

**Conséquence sur le thème.** Cette liste devient un ensemble de rôles nommés dans les jetons de thème, pas une suite de valeurs recopiées. Quand un utilisateur change la couleur d'accentuation, tous ces éléments suivent ensemble, et la vérification automatique du contraste s'applique à chacun.
