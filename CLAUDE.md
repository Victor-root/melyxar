# Melyxar

Serveur multimédia en Rust (films et séries), dans l'esprit de Jellyfin et Emby, avec une base propre et une exigence forte de réactivité. Tourne dans un LXC Debian 13 non privilégié sur Proxmox. Premier client : le navigateur (Brave / Chromium).

## À lire avant tout travail

- `docs/architecture/README.md` : décisions actées, environnement de production, règles de travail.
- `docs/architecture/01-revue-architecture.md` : architecture complète (crates, modèle de données, moteur de lecture).
- `docs/architecture/02-reactivite.md` : exigence de réactivité, chemin chaud / chemin froid, budgets.
- `docs/architecture/03-plan.md` : plan par jalons et état d'avancement. Le mettre à jour à chaque jalon terminé.

Toute nouvelle décision d'architecture est ajoutée au README des décisions avant d'être codée.

## Le mainteneur

- Ne lit pas le code. Lui parler en français, en termes humains, sans jargon inutile. Réponses concises lors des itérations.
- Compile et déploie lui-même dans le LXC de production via la commande de mise à jour. Il rapporte les erreurs et les journaux.
- Ses PC sont sous Windows : rien ne doit exiger un poste Linux en dehors du LXC.

## Langues

- **Code en anglais** : crates, modules, types, fonctions, variables, tables, colonnes, routes, messages de commit, commentaires, journaux techniques.
- **Tout le reste en français** : documentation, explications, réponses au mainteneur.
- Textes d'interface toujours via l'internationalisation : anglais d'abord, français ensuite.
- Le em dash est banni de tout texte, code compris.

## Règles de code

- Respecter l'architecture des documents : direction unique des dépendances entre crates, SQL uniquement dans `database`, binaires FFmpeg lancés uniquement par `ffmpeg`, logique métier dans `app` et jamais dans les handlers HTTP, décision de lecture pure dans `playback`.
- SQLite en mode WAL, une seule connexion d'écriture, transactions courtes.
- **Toute requête SQL doit être couverte par un test d'intégration** sur une base migrée : c'est ce qui remplace la vérification à la compilation.
- Aucun travail lourd sur les threads asynchrones de Tokio.
- Tout journal de débogage est protégé par `cfg(debug_assertions)` ou équivalent, jamais actif en release.
- **Journaux : les noms de médias apparaissent en clair**, fichier et chemin complet. Ce journal n'est vu que par le mainteneur ; le censurer devant lui-même ne servait à rien, et le réglage qui permettait de le faire a été retiré.
- **Rien de personnel au mainteneur ne doit apparaître dans le dépôt ni sur GitHub, jamais** : ni nom de fichier média réel, ni chemin réel, ni numéro de série de disque, que ce soit dans le code, les commentaires de code, les tests, la documentation, un message de commit, ou le titre et la description d'une pull request ou d'un commentaire GitHub. Ce qui est discuté en conversation avec le mainteneur reste entre lui et Claude et ne doit jamais être recopié dans le dépôt ou publié. Seule exception, volontaire et à ne pas défaire : la clé du fournisseur de métadonnées, qui identifie l'application et non le mainteneur, voyage brouillée dans les valeurs par défaut de la crate `metadata`. Les jeux de tests utilisent des titres inventés couvrant les mêmes formes, et les chemins réels vivent uniquement dans la configuration du serveur.
- **L'interface web est toujours à jour toute seule.** Personne ne doit jamais recharger la page pour voir l'état réel : position de reprise dès qu'on quitte un film, fiche corrigée dès qu'elle est identifiée, réglage enregistré, travail en cours qui avance, bibliothèque qui grossit pendant un scan. Une action met à jour tous les écrans qui la montrent, y compris celui d'où elle est partie et ceux qu'on retrouve en revenant en arrière.
- **Aucune couleur en dur dans l'interface, le rouge moins que tout autre.** Le rouge de Melyxar n'est que la valeur par défaut de la couleur d'accentuation, que chaque compte peut changer. Tout ce qui est coloré passe par les jetons de `web/src/theme.css` : l'accentuation et ses nuances pour ce qui est actif, sélectionné, en progression ou actionnable, et des jetons d'état nommés pour ce qui va bien, ce qui demande attention et ce qui est en panne. Une valeur écrite directement dans un composant ou une feuille de style ne suit ni le choix de la personne ni le thème clair.
- Pas de code mort, pas de contournement temporaire, pas de commentaire inutile. Nettoyer entièrement toute tentative abandonnée.
- Vérifier les usages réels avant de supprimer, déplacer ou remplacer du code.
- Avant chaque commit : **toujours** compiler et relire le diff complet. Les tests, eux, se dosent selon ce qui bouge, parce qu'une suite complète prend plusieurs minutes et qu'on ne la paie que quand elle peut trouver quelque chose :
  - **Apparence seule** (CSS, mise en page, espacement, couleurs, formulations) : pas de tests. Le typecheck et le build de l'interface suffisent. Un test n'attrape rien là-dessus, c'est l'œil du mainteneur qui juge, et le lui faire attendre ne sert personne.
  - **Un calcul, où qu'il vive, l'interface comprise** : le test de cette unité. Une formule fausse a l'air juste à l'écran, donc l'œil ne l'attrape pas. Constaté : l'étiquette « % de l'image gardée » comptait la barre du haut dans la bannière bien après que ça ait cessé d'être vrai, et le mainteneur a réglé un curseur sur un chiffre faux. Côté interface, le lanceur est Vitest : `npm --prefix web test`, moins d'une seconde pour toute la suite, donc il n'y a jamais de raison de s'en passer. Le fichier de test vit à côté de ce qu'il teste, en `.test.ts`.
  - **Le moteur** (logique Rust, requête SQL, migration, quoi que ce soit qui traverse plusieurs crates) : `cargo clippy` et la suite complète, sans exception. C'est là que les fautes sont invisibles à la lecture et coûteuses. Constaté deux fois le même jour : un paramètre manquant dans une requête d'insertion, que seule la suite a attrapé.

## Git

- Branche de travail : `develop`. Ne jamais toucher à `main` sans demande explicite. Ne jamais pousser une branche `claude/...`.
- Commits petits et descriptifs, en anglais. Commiter chaque étape terminée avant de passer à la suivante.
- Pas d'intégration continue GitHub : la vérification se fait ici avant commit, puis dans le LXC par le mainteneur.

## Environnement de production (résumé)

- LXC Debian 13 non privilégié, 12 threads, 16 Go, 100 Go NVMe. Hôte Proxmox avec Intel Arc A380 passée via `/dev/dri`.
- Médias : quatre disques montés sous `/mnt/`, chacun avec des dossiers `Films`, `Séries`, `Animés`, `Émissions`, lisibles par tous. Films posés à plat, sans sous-dossier par film. Une bibliothèque regroupe les quatre dossiers de même nom.
- Accès local en `ip:2100` pour commencer ; reverse proxy nginx (autre LXC) plus tard.
- Configuration en TOML dans `/etc/melyxar/melyxar.toml` ; données dans `/var/lib/melyxar` ; cache dans `/var/cache/melyxar`.
