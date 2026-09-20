# Melyxar, la réactivité

Second volet de la revue d'architecture, septembre 2026. Exigence ajoutée : **une bibliothèque déjà indexée doit donner une sensation de navigation quasi instantanée, même pendant un scan, un téléchargement de métadonnées ou un transcodage.**

## Verdict global

**La réactivité ne vient ni du langage, ni de la base de données. Elle vient de la discipline : rien de lourd sur le chemin de lecture.** Ouvrir une grille ou une fiche déjà connue ne doit toucher que des lignes indexées et des fichiers image déjà prêts. Tout le reste (ffprobe, TMDb, redimensionnement, comptages) se fait à l'écriture, en arrière-plan, avec des limites strictes de parallélisme.

Jellyfin et Emby utilisent tous les deux SQLite et tous les deux .NET. La différence ressentie ne peut donc pas venir de là. Elle vient très probablement de la quantité de travail par requête, de la forme du client web et de l'état des caches, et cela se vérifie en dix minutes avec les outils de Brave.

## 1. Deux réponses préalables

### SQLite, tranché pour de bon

Avec le contexte (le mainteneur ne code pas, compile et teste, veut une navigation instantanée), **SQLite** sans hésiter :

- **Un seul binaire, une seule chose qui peut casser.** Pas de service PostgreSQL à installer, configurer, sauvegarder et mettre à jour. Sauvegarder la base, c'est copier un fichier. Diagnostiquer, c'est envoyer ce fichier.
- **Plus rapide pour ce type d'usage.** SQLite tourne dans le processus du serveur : une lecture indexée coûte quelques microsecondes, sans aller-retour réseau. Une page qui fait dix petites requêtes reste sous la milliseconde.
- **Emby et Plex prouvent l'échelle.** Les deux tournent sur SQLite avec des bibliothèques de centaines de milliers d'éléments.

Ce que PostgreSQL apportait existe dans SQLite : JSON1 et FTS5. Les modifications de tables limitées se contournent avec le schéma classique (créer la nouvelle table, copier, renommer, dans une transaction).

**Les trois règles SQLite non négociables :**
1. Mode journal WAL : les lecteurs ne sont jamais bloqués par l'écrivain.
2. Une seule connexion d'écriture, dédiée ; un petit groupe de connexions de lecture. Jamais deux écrivains en concurrence.
3. Des transactions d'écriture courtes, par lots (quelques centaines de lignes), qui ne contiennent jamais un appel à ffprobe ou à TMDb.

### Compilation et ressources : tout se passe dans le LXC

Les PC du mainteneur sont sous Windows et le LXC est facile d'accès : **la compilation se fait directement dans le LXC de production.** On y installe une fois les outils Rust et Node, puis une seule commande met Melyxar à jour : récupérer le dépôt, compiler, appliquer les migrations, redémarrer le service. Pendant la compilation, le serveur en cours continue de tourner ; seul le redémarrage final coupe la lecture quelques secondes.

| Poste | Quoi | Ressources |
|---|---|---|
| Compilation dans le LXC | Construire le serveur Rust et l'interface web | Pic de 6 à 8 Go de mémoire. 5 à 10 minutes la première fois, 1 à 2 minutes ensuite. 15 à 20 Go de disque pour les fichiers intermédiaires. |
| Navigation seule | Servir l'interface, la base, les images | Une fraction de cœur, 100 à 300 Mo. |
| Scan de bibliothèque | ffprobe, TMDb, redimensionnement | 2 à 4 cœurs suffisent, dans les limites fixées par le serveur. |
| Transcodage logiciel | FFmpeg sans carte graphique | 4 à 8 cœurs par flux 1080p ; un flux 4K HDR sature un Ryzen 3700X entier. |
| Transcodage matériel | FFmpeg avec l'Intel Arc A380 | Moins d'un cœur par flux. |

**LXC retenu : 100 Go de disque, 16 Go de mémoire, 12 threads** (monter à 16 si les autres services de l'hôte le permettent ; le gain ne concerne que la compilation). Le 16 Go se justifie par la compilation, pas par le serveur. Deux précautions : la compilation tourne en priorité basse pour ne pas faire saccader une lecture en cours, et les métadonnées SQLx restent versionnées dans le dépôt pour que la compilation ne dépende jamais de l'état de la base. Plus tard, GitHub pourra compiler automatiquement et publier un exécutable prêt ; ce sera la bonne façon de distribuer Melyxar à d'autres.

**L'Intel Arc A380** est l'une des meilleures cartes bon marché pour un serveur multimédia : décodage et encodage H.264, HEVC et AV1, tonemapping HDR sur la carte. Elle demande un noyau récent sur l'hôte Proxmox, le pilote Intel dans le LXC, et le passage de `/dev/dri` au conteneur. La distribution FFmpeg de Jellyfin est la référence pour cette carte.

## 2. Jellyfin contre Emby : faits et hypothèses

Emby est fermé depuis 2018 : on ne peut pas lire son code.

### Ce qui est vérifiable

- **Les deux utilisent SQLite.** L'hypothèse « c'est SQLite qui est lent » est réfutée par le fait qu'Emby est rapide dessus.
- **Les deux tournent sur .NET.** Ce n'est pas le langage du backend.
- **Le client web de Jellyfin descend de celui d'Emby** (copie de la dernière version ouverte, fin 2018), puis sept ans de divergence.
- **Jellyfin redimensionne les images à la demande**, puis les met en cache. La première visite d'une page est plus lente. Si le cache est sur un disque lent, chaque première visite reste lente.
- **Un navigateur limite à environ six connexions simultanées par serveur en HTTP/1.1.** Une grille de soixante affiches, c'est dix vagues de six requêtes. En HTTP/2 (reverse proxy avec TLS), la limite disparaît.
- **Jellyfin a refait sa couche base de données récemment**, avec des régressions de performance rapportées pendant la transition.

### Ce qui est plausible sans être prouvé

| Hypothèse | Statut | Comment la vérifier |
|---|---|---|
| Jellyfin fait plus de travail par requête | Plausible | Onglet Réseau de Brave : temps de réponse de `/Items` à l'ouverture d'une bibliothèque. Au-delà de 200 ms pour cent éléments, le serveur travaille trop. |
| Le client web de Jellyfin est plus lourd | Plausible | Onglet Performance : si le temps est dans le scripting et le rendering plutôt que dans l'attente réseau, c'est le client. Compter les requêtes au retour en arrière. |
| Emby met plus en cache côté client et précharge | Invérifiable | Observer le nombre de requêtes et celles servies depuis le cache navigateur. |
| Les deux installations ne sont pas comparables | À exclure d'abord | Cache et base sur le NVMe dans les deux cas ? Même reverse proxy ? Plugins ? Base Jellyfin gonflée par le journal d'activité ? |
| Jellyfin ralentit pendant les scans | Souvent rapporté | Comparer `/Items` avec et sans scan. |

### Le protocole de dix minutes

1. Brave, page d'accueil de Jellyfin, outils de développement (F12), onglet Réseau, cache activé.
2. Ouvrir une bibliothèque de films. Noter : nombre de requêtes, temps de `/Items`, temps de la première et de la dernière image.
3. Revenir à l'accueil, rouvrir la même bibliothèque : état « chaud ». Si le chaud est lent, le problème est structurel. Si seul le froid est lent, c'est le cache d'images.
4. Refaire la même chose sur Emby.
5. Pendant un scan Jellyfin, refaire l'étape 2.

Ces chiffres deviennent les cibles de Melyxar.

## 3. Le principe : chemin chaud, chemin froid

| Chemin chaud (à chaque clic, moins de 50 ms) | Chemin froid (en arrière-plan, sans budget) |
|---|---|
| Navigateur : grille virtualisée, cache des réponses, images gardées par le navigateur | Scan et identité des fichiers : parcours, détection des ajouts, renommages, absences |
| API de navigation : une requête par écran, réponse légère, ETag, pagination par curseur | ffprobe, TMDb, images sources : parallélisme borné, priorité basse, annulable |
| Lecture indexée : colonnes de navigation, tri par index, aucun calcul, aucun décodage | Précalcul : titre de tri, couleur dominante, tailles d'images, comptages, index de recherche |
| Images déjà prêtes : fichiers WebP aux tailles fixes, URL avec empreinte, cache un an | Écriture par lots : transactions courtes, une seule connexion d'écriture, événement « bibliothèque modifiée » |

Ce que les deux partagent : la base SQLite en mode WAL et le cache d'images, sur le NVMe. Le chemin froid écrit, le chemin chaud lit.

**La règle qui résume tout : si une opération peut être faite avant que l'utilisateur clique, elle est faite avant.**

Seuils de perception : en dessous de **100 ms**, une réaction paraît instantanée ; jusqu'à **une seconde**, l'utilisateur garde le fil mais sent l'attente. « Quasi instantané » : chaque clic sur des données connues affiche quelque chose d'utile en moins de 100 ms, le reste arrive progressivement sans faire sauter la mise en page.

## 4. Côté serveur, jusqu'à 100 000 médias

### Deux formes de données : la carte et la fiche

Une grille affiche des **cartes** (titre, année, affiche, note, progression, vu ou non). Une **fiche** affiche le reste (synopsis, distribution, pistes, versions).

- Les colonnes de carte sont des colonnes indexées, pas un bloc JSON à décoder : titre de tri précalculé (sans « Le », « The », accents normalisés), année, note, durée, date d'ajout, identifiant de l'affiche, couleur dominante.
- Synopsis, distribution et réponses brutes des fournisseurs vivent à part, chargés uniquement par la fiche.
- La réponse de liste contient tout ce qu'il faut pour dessiner la carte, y compris l'URL finale de l'image et l'état de lecture. Une grille de cent cartes, c'est une requête.
- Chaque carte pèse quelques centaines d'octets. Cent cartes : 30 Ko, compressés en 8.

### Tri, filtre, pagination

- Un index par tri proposé (titre, date d'ajout, année, note), combiné à la bibliothèque.
- Pagination par curseur (« les cent suivants après ce titre ») plutôt que par décalage : instantané au 90 000e élément. Gratuit dès le début, coûteux à changer après.
- Filtres courants (genre, décennie, non vu, résolution) par tables de liaison indexées.
- Comptages recalculés à l'écriture et stockés, jamais comptés à la lecture.
- Recherche par FTS5, alimenté à l'écriture, recherche par préfixe pour la saisie en direct.

### Les images : le poste qui décide tout

- Générer à l'écriture un jeu de tailles fixes (par exemple 240, 480, 960 px de large pour les affiches, deux tailles pour les fonds), en WebP. Pour 100 000 films : environ 10 Go de cache sur le NVMe.
- Des URL qui portent une empreinte du contenu : l'URL change quand l'affiche change, donc le navigateur peut garder l'image un an. Mécanisme le plus rentable de la liste.
- Une couleur dominante ou un flou miniature stockés en base et renvoyés dans la liste : la grille s'affiche colorée avant la première image, sans sauts de mise en page.
- Jamais de redimensionnement sur le chemin chaud. Si une taille manque, renvoyer la plus proche et planifier la génération en tâche de fond.
- HTTP/2 : déploiement recommandé derrière un reverse proxy (Caddy) avec TLS, et prévoir que le serveur puisse un jour le servir lui-même.

### Le cache serveur : moins qu'on ne croit

Avec SQLite sur NVMe et des index bien posés, le cache du système tient la base en mémoire. Un cache applicatif générique serait prématuré. Deux exceptions dès le début :
- ETags sur les réponses de l'API.
- Un numéro de version par bibliothèque, incrémenté à chaque écriture, qui sert aux ETags, à l'invalidation du cache client et aux notifications temps réel.

## 5. Empêcher les travaux de fond de ralentir la navigation

| Ressource | Parades |
|---|---|
| Processeur | Aucun travail lourd sur les threads asynchrones de Tokio. ffprobe est un processus externe ; le redimensionnement passe par le pool bloquant limité. Sémaphores : par exemple deux ffprobe, deux redimensionnements, quatre appels TMDb. Processus de fond lancés en priorité basse (`nice`) ; les transcodages de lecture restent en priorité normale. Avec l'Arc A380, le transcodage ne consomme presque pas de CPU. |
| Base de données | WAL. Transactions courtes (100 à 500 lignes), jamais ouvertes pendant un appel réseau ou un ffprobe. Connexion d'écriture unique, propriété de `database`, avec une file. Le scan écrit en flux, pas en une transaction finale géante. |
| Disque | Base et cache d'images sur le NVMe ; segments de transcodage dans un dossier séparé, idéalement tmpfs, avec quota. Génération d'images par lots avec pause si le disque est sollicité. |
| Mémoire | Le scan traite en flux, jamais « charger toute la liste puis traiter ». Pas de cache d'images en mémoire. Limite de sessions de transcodage simultanées. |

**Le piège classique.** Un scan qui, en fin de parcours, exécute « marquer tout ce qui n'a pas été vu comme absent » en une seule requête sur 100 000 lignes et bloque l'écrivain plusieurs secondes : les écritures de progression s'empilent. Parade : marquer par lots, et ne jamais faire dépendre une réponse HTTP d'une écriture qui peut attendre (la progression est mise en file et confirmée immédiatement).

## 6. Côté client

- **Cache des réponses et rafraîchissement silencieux.** TanStack Query garde chaque réponse, l'affiche immédiatement au retour, revalide en arrière-plan avec l'ETag. Le numéro de version de bibliothèque (WebSocket ou événements serveur) invalide exactement ce qui a changé.
- **Préchargement ciblé.** Survoler une carte 100 ms précharge sa fiche et son fond. Ouvrir une bibliothèque précharge la page suivante aux deux tiers du défilement. L'accueil précharge les premières cartes de chaque section visible, pas plus. Jamais « toute la bibliothèque ».
- **Virtualisation des grilles.** Une grille de 5 000 cartes n'existe dans la page que pour la centaine visible. Indispensable dès quelques centaines d'éléments, donc dès la V0.1. Les cartes ont une taille prévisible.
- **Images adaptées.** `srcset` et `sizes` pour demander la taille juste au-dessus de l'affichage réel. Chargement paresseux natif hors écran, priorité haute pour le visible. Couleur dominante en fond immédiat, aucun saut de mise en page.
- **Ce qui ne doit pas être fait côté client.** Trier ou filtrer en mémoire une liste chargée en entier, dupliquer la logique métier, maintenir une copie locale de la base. Le client est un cache d'affichage, pas une seconde source de vérité.

## 7. Dès le début ou prématuré

### Dès la V0.1

| Quoi | Pourquoi maintenant |
|---|---|
| Carte et fiche comme deux formes distinctes | Change la forme de l'API et du schéma. |
| Colonnes de navigation indexées, titre de tri précalculé | Gratuit dans la première migration. |
| Pagination par curseur | Même coût au départ, incompatible après. |
| Images pré-générées, URL avec empreinte, cache un an | Décide la structure du cache et des routes d'images. |
| Couleur dominante dans la réponse de liste | Quelques octets par carte, calculés à l'écriture. |
| WAL, écrivain unique, transactions courtes | Discipline de code dans `database`. |
| Parallélisme borné et priorité basse des tâches de fond | Quelques lignes dans `jobs`. |
| Rien de lourd sur les threads asynchrones | Règle de revue de code. |
| Version de bibliothèque et ETags | Deux colonnes et un en-tête. |
| Virtualisation des grilles et cache des réponses côté client | Fondations du frontend. |
| Journalisation des temps par requête et générateur de bibliothèque synthétique | Sans mesure dès le premier jour, on ne saura jamais quand on a régressé. |

### Prématuré, voire nuisible

| Quoi | Pourquoi attendre |
|---|---|
| Cache applicatif générique en mémoire (ou Redis) | SQLite sur NVMe et le cache du système font mieux. À reconsidérer si une requête chaude dépasse 20 ms. |
| Précalculer toutes les combinaisons de filtres | Explosion combinatoire. Les index suffisent. |
| Rendu côté serveur, service worker hors ligne, HTTP/3 | Complexité réelle, gain nul en réseau local. |
| AVIF | WebP suffit, encode dix fois plus vite. |
| Copie locale de la base dans le navigateur | Double source de vérité. |
| Serveur d'images dédié | Des fichiers statiques sur NVMe sont déjà au plancher. |
| Optimiser le scan à l'extrême | Un scan lent mais isolé ne gêne personne. |

## 8. Mesurer et détecter les régressions

### Budgets (bibliothèque de 100 000, 95e centile)

| Mesure | Cible | Où on la lit |
|---|---|---|
| Page de grille (100 cartes), côté serveur | moins de 30 ms | Journal du serveur, en-tête Server-Timing |
| Fiche complète, côté serveur | moins de 20 ms | Idem |
| Menus d'une médiathèque (genres, décennies, lettres), côté serveur | moins de 30 ms | Idem |
| Page d'accueil, côté serveur | moins de 30 ms | Idem |
| Image en cache, temps avant premier octet | moins de 5 ms | Onglet Réseau de Brave |
| Recherche par préfixe, côté serveur | moins de 30 ms | Journal du serveur |
| Grille visible, retour sur page connue | moins de 100 ms | Mesure dans le client |
| Grille visible, première visite | moins de 500 ms en réseau local | Idem |
| Ces mêmes chiffres pendant un scan | au plus 1,5 fois les cibles | Banc d'essai |

### Instrumentation dès la V0.1

- Un temps par requête dans les journaux : route, durée totale, durée en base, nombre de requêtes SQL. Le nombre de requêtes SQL par page est l'indicateur le plus précieux (détecte le « N+1 »).
- L'en-tête Server-Timing sur chaque réponse, visible dans Brave.
- Des histogrammes exposés (format Prometheus) pour les routes, les tâches de fond et la file d'écriture.
- Le client mesure le temps entre le clic et la grille affichée, et l'envoie au serveur.

### Banc d'essai synthétique

Fait, et intégré au serveur plutôt que confié à un outil de charge extérieur : le banc doit tourner chez le mainteneur, sur sa machine, sans rien à installer.

- `melyxar bench fill --works 100000` écrit une bibliothèque inventée à côté des vraies, dans la forme qu'une vraie a : titres répartis sur tout l'alphabet, soixante ans de sorties, genres et studios partagés comme un catalogue les partage, quelques milliers d'acteurs crédités sur l'ensemble, un sixième des œuvres en séries avec leurs saisons et leurs épisodes. Les noms partagés qui existent déjà sont réutilisés, jamais réécrits.
- Aucun fichier n'est écrit sur un disque. Les affiches existent en lignes et rien de plus : ce qu'un fichier coûte à servir ne change pas avec la taille de la collection, donc en écrire cent mille ne mesurerait rien de neuf.
- `melyxar bench run` joue les pages qu'un spectateur ouvrirait, deux cents fois chacune, et met ses centiles en face des budgets ci-dessus. Il sort en erreur quand un budget est dépassé.
- `melyxar bench empty` retire la bibliothèque inventée et rend la place au disque.
- Entrée de menu dans le script d'installation, qui enchaîne les trois.

Reste à faire : la mesure pendant un scan et deux transcodages, qui demande une vraie collection, et le test automatisé de l'interface (Playwright), qui attend l'interface définitive.

### Ce que le banc a trouvé

Trois défauts qu'une collection de cinquante films ne pouvait pas montrer, tous corrigés le jour où ils ont été mesurés :

1. **Les menus d'une médiathèque** prenaient 690 ms et **la page d'accueil** 156 ms, parce que toutes deux comptaient la collection entière à chaque visite. Ces comptes sont maintenant faits une fois par changement et relus ensuite.
2. **Le dernier arrivé, toutes médiathèques confondues** n'était couvert par aucun index : tous ceux qui ordonnent les œuvres commencent par la médiathèque.
3. **Retirer une grosse médiathèque prenait trois minutes**, pendant lesquelles rien d'autre ne pouvait être écrit. La base suit la suppression dans chaque table qui pointe vers ce qui s'en va, et une colonne qui pointe sans index se lit en entier, une fois par ligne retirée. Vingt-deux secondes une fois les index posés.
4. **Filtrer par décennie** demandait une tranche d'années, et une tranche ne se lit pas dans l'ordre des titres : toute la décennie était lue et triée avant d'envoyer cent cartes. Tenu à 100 000 (17 ms), dépassé à 500 000 (82 ms pour 30 de budget). La décennie est maintenant une valeur que la base calcule elle-même à partir de l'année, et l'index la porte devant le titre : 5 ms, et le même chiffre aux deux tailles.

Le troisième donne une règle générale, tenue par un test qui parcourt le schéma : **toute colonne qui pointe vers une autre table a un index**. C'est la règle de SQLite elle-même, et c'est celle qu'on oublie, parce que rien ne s'en plaint tant que les tables sont petites.

Le quatrième en donne une autre : **ce qui restreint une grille doit être une valeur, pas une tranche**, sinon l'index ne peut pas servir à la fois à choisir et à ordonner, et le temps se met à suivre la taille de la collection.

Mesuré à 100 000 œuvres, la cible, et vérifié à 500 000, cinq fois la cible : tous les budgets tenus, et les mêmes chiffres aux deux tailles.

### Détection des régressions

Le banc tourne en intégration continue à chaque changement, sur la base de 50 000, et compare aux centiles de référence. **Une dégradation de plus de 20 % sur une cible fait échouer la vérification**, en nommant la route.

## 9. Outils pour un mainteneur non-développeur

- **Une commande de diagnostic** (`melyxar doctor`) : version de FFmpeg et accélérations détectées, accès à `/dev/dri`, permissions sur chaque racine, mode WAL, taille de la base et du cache, temps des cinq requêtes de référence. Résultat à coller dans une conversation.
- **Une page « Diagnostic » dans l'interface** avec les mêmes informations, plus les tâches de fond, la file d'écriture, les sessions de lecture et leurs décisions.
- **Des journaux structurés lisibles** : une ligne par requête avec sa durée, une par décision de lecture avec ses raisons, une par lot de scan. Niveau réglable sans recompiler.
- **Un export de diagnostic en un clic** : journaux récents, résultat de la commande, configuration sans les secrets.
- **La base SQLite elle-même** : un fichier à copier et envoyer, ou à ouvrir avec un outil graphique.

L'exigence de réactivité ne coûte presque rien si elle est posée maintenant. Elle coûterait une réécriture si elle arrivait après.
