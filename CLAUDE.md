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
- **Journaux : les noms de médias sont censurés.** Un nom de fichier n'apparaît que par ses quatre premiers caractères suivis de points de suspension ; les chemins n'affichent que le libellé de la racine. Un réglage de configuration explicite, désactivé par défaut, permet de révéler les noms complets.
- **Rien de personnel au mainteneur ne doit apparaître dans le dépôt** : ni nom de fichier média réel, ni chemin réel, ni numéro de série de disque, ni clé d'API, que ce soit dans le code, les tests, la documentation ou un message de commit. Les jeux de tests utilisent des titres inventés couvrant les mêmes formes, et les chemins réels vivent uniquement dans la configuration du serveur.
- Pas de code mort, pas de contournement temporaire, pas de commentaire inutile. Nettoyer entièrement toute tentative abandonnée.
- Vérifier les usages réels avant de supprimer, déplacer ou remplacer du code.
- Avant chaque commit : compiler, lancer `cargo clippy` et les tests dans l'environnement de travail. Relire le diff complet.

## Git

- Branche de travail : `develop`. Ne jamais toucher à `main` sans demande explicite. Ne jamais pousser une branche `claude/...`.
- Commits petits et descriptifs, en anglais. Commiter chaque étape terminée avant de passer à la suivante.
- Pas d'intégration continue GitHub : la vérification se fait ici avant commit, puis dans le LXC par le mainteneur.

## Environnement de production (résumé)

- LXC Debian 13 non privilégié, 12 threads, 16 Go, 100 Go NVMe. Hôte Proxmox avec Intel Arc A380 passée via `/dev/dri`.
- Médias : quatre disques montés sous `/mnt/`, chacun avec des dossiers `Films`, `Séries`, `Animés`, `Émissions`, lisibles par tous. Films posés à plat, sans sous-dossier par film. Une bibliothèque regroupe les quatre dossiers de même nom.
- Accès local en `ip:2100` pour commencer ; reverse proxy nginx (autre LXC) plus tard.
- Configuration en TOML dans `/etc/melyxar/melyxar.toml` ; données dans `/var/lib/melyxar` ; cache dans `/var/cache/melyxar`.
