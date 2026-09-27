# Partage de médiathèques entre serveurs

État : **idée future, rien n'est codé.** Ce document fixe le concept discuté avec le mainteneur pour le jour où il sera construit. Rien ici n'est encore une décision actée du README : les choix passeront dans le tableau des décisions au moment de coder.

## Le principe

Deux serveurs Melyxar s'appairent, puis chacun choisit explicitement quelles médiathèques il autorise l'autre à consulter. Chaque serveur peut ensuite ajouter une médiathèque autorisée de l'autre **comme un dossier de plus** d'une de ses propres médiathèques.

Exemple : mon serveur a une médiathèque Films, un serveur appairé m'autorise la sienne. Je l'ajoute comme dossier de ma médiathèque Films : les films de l'autre serveur s'affichent dans ma médiathèque Films, avec les miens. Chaque fichier reste physiquement sur son serveur d'origine, qui reste responsable du fichier, de FFmpeg et de toute la lecture. L'origine distante est indiquée discrètement dans l'interface.

Trois consentements distincts :

1. **L'appairage** ne partage rien.
2. **L'autorisation** se donne médiathèque par médiathèque, et peut être asymétrique : on peut prêter sans recevoir.
3. **L'ajout** d'une médiathèque distante est le choix de celui qui reçoit. Par défaut dans sa médiathèque du même type ; il peut aussi en faire une médiathèque à part s'il le souhaite.

Aucune responsabilité sur le contenu partagé n'est prise ni discutée par Melyxar : chaque administrateur partage ce qu'il veut.

## Une médiathèque distante est un dossier de plus

Une médiathèque Melyxar regroupe déjà plusieurs dossiers, un par disque. Une médiathèque distante en devient un dossier d'un genre à part : ses fichiers vivent ailleurs, on ne le parcourt pas comme un disque, on le synchronise. Presque tout le reste suit sans changer :

- grilles, recherche, rangées de l'accueil, accès par compte : le contenu distant est dans la médiathèque comme le reste ;
- **un même film présent des deux côtés est une seule fiche avec deux versions**, par le sélecteur de versions qui existe déjà. Reconnu par l'identifiant du fournisseur, jamais par le titre ; une œuvre non identifiée reste une carte à part ;
- une série dont une saison est ici et une autre là-bas est une seule série ;
- l'état d'un dossier (introuvable, illisible...) gagne un cas : **serveur injoignable**. Ses œuvres restent visibles, marquées indisponibles, comme celles d'un disque débranché.

Seules les œuvres de même type se rejoignent : une médiathèque distante d'animés ne s'ajoute qu'à une médiathèque d'animés.

## Ce que chaque serveur tient

| | Serveur qui reçoit | Serveur d'origine |
|---|---|---|
| Tient | une copie locale du catalogue distant, ses comptes, leurs marques et leur progression | les fichiers, les fiches qui font foi, les images |
| Fait | la navigation, la recherche, le relais de la lecture | FFmpeg, la décision de lecture, sous-titres, vignettes, génériques |
| Voit | ce que ses comptes font | **qui lit quoi et où il en est**, comme pour un compte local |

**La copie locale du catalogue est indispensable.** La navigation tient ses budgets (quelques millisecondes à 100 000 œuvres) parce qu'elle ne lit que la base locale. Interroger l'autre serveur à chaque page la rendrait lente et dépendante d'internet. Le catalogue distant est donc recopié en arrière-plan, par différences (« qu'est-ce qui a changé depuis la dernière fois »), en s'appuyant sur le compteur de version que chaque médiathèque tient déjà. Les images sont recopiées de la même façon. Seule la lecture d'une vidéo va chez l'origine.

## Les invités sont connus de l'origine

L'administrateur d'origine doit savoir précisément ce qui se passe sur son serveur. Chaque compte du serveur qui reçoit est donc représenté chez l'origine par un **compte invité** :

- nommé avec son serveur (« Victor, serveur de Victor »), identifié par le couple serveur et identifiant du compte, **jamais par son nom** : deux Victor sur deux serveurs sont deux personnes (leçon de GlassKeep) ;
- visible dans les lectures en cours, le journal et l'activité, avec ce qu'il regarde et où il en est, comme un compte local ;
- soumis aux règles de l'origine : l'administrateur peut le limiter ou le couper, lui seul ou tout le serveur appairé ;
- incapable de se connecter directement : il n'existe qu'à travers son serveur.

La progression vit aux deux endroits, chacun pour son usage : le serveur qui reçoit la garde pour ses rangées et ses marques, sans quoi la navigation dépendrait de l'autre ; l'origine la reçoit en direct pendant la lecture pour que son administrateur la voie.

## La lecture passe par le serveur qui reçoit

Le navigateur ne parle qu'à son serveur, qui ouvre une session de lecture chez l'origine au nom du compte invité, lui transmet ce que le navigateur sait lire, et relaie les morceaux de vidéo. Un seul site pour le navigateur (le cookie de connexion reste valable), l'origine n'est pas exposée aux appareils de l'autre maison, et le surcoût d'une étape est négligeable avec des morceaux de quatre secondes.

## Débit et transcodage

- **Plafond de débit par partage, facultatif**, fixé par l'origine en autorisant la médiathèque. Le serveur qui reçoit en est informé et l'applique d'avance : la qualité proposée ne dépasse pas le plafond, et le lecteur dit pourquoi. C'est une exception assumée à la règle « la version n'est jamais choisie selon la connexion », limitée au contenu distant.
- **Aucune limite de transcodage simultané par défaut**, pour les comptes locaux comme pour les invités. Une limite reste possible en option, et l'origine peut en fixer une propre à un partage.

## Les fiches

L'origine a raison sur ses fiches. Le serveur qui reçoit peut proposer une modification (titre, identification, image, générique) : elle devient une **demande** envoyée à l'administrateur d'origine, qui la voit dans ses notifications et l'accepte ou la refuse. Acceptée, elle est appliquée chez l'origine et revient par la synchronisation ; en attendant, elle est marquée « en attente » chez celui qui l'a proposée. Une demande porte la version de la fiche qu'elle modifie, pour qu'une demande devenue périmée soit signalée plutôt qu'appliquée par-dessus.

Ne traversent jamais : la suppression sur disque, et toute action qui écrit ou lit un fichier de l'origine autrement que par la lecture. Relancer une analyse ou un entretien sur une œuvre distante est une demande, comme une modification de fiche.

## Appairage et sécurité

- **HTTPS obligatoire des deux côtés** : le jalon 9 est un préalable.
- Appairage par un **code à usage unique** affiché par un administrateur et saisi par l'autre ; chacun confirme. Chaque serveur a sa propre **paire de clés** et signe ses requêtes ; aucun secret n'est stocké en clair, seule une empreinte l'est, comme pour les sessions.
- **Protection contre le rejeu** : une requête signée n'est acceptée qu'une fois, pas seulement dans une fenêtre de temps.
- **Aucune adresse interne** acceptée comme adresse d'un serveur appairé, et aucune requête sortante déclenchée par une demande anonyme.
- Rotation des clés sans réappairer ; désappairer retire la copie du catalogue et les comptes invités, sans toucher aux marques des comptes locaux sur leurs propres œuvres.
- Un **numéro de version du protocole** entre serveurs, indépendant de la version de Melyxar, pour que deux serveurs mis à jour à des dates différentes continuent de se parler et disent clairement ce qui leur manque.
- L'origine n'envoie jamais un chemin complet de fichier, seulement ce dont l'autre a besoin pour afficher et demander la lecture.

## Ordre de construction envisagé

1. Préalables : HTTPS (jalon 9) et une spécification stable de l'API.
2. Appairage, autorisations, comptes invités.
3. Copie du catalogue et médiathèque distante comme dossier, en lecture seule, œuvres distantes séparées des locales.
4. Lecture relayée, avec plafond de débit.
5. Réunion des œuvres présentes des deux côtés par identifiant du fournisseur.
6. Demandes de modification de fiche.

## Questions encore ouvertes

- Ce que voit un compte de l'origine quand l'œuvre existe aussi chez lui en local : une fiche ou deux ?
- Faut-il pouvoir masquer entièrement les œuvres d'un serveur injoignable plutôt que les marquer indisponibles ?
- Les génériques détectés : une série dont les saisons sont réparties sur deux serveurs est écoutée saison par saison chez chacun, ce qui suffit a priori, à vérifier.
