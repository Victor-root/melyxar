# Melyxar

Serveur multimédia en Rust, films et séries, dans l'esprit de Jellyfin et Emby,
avec une exigence forte de réactivité. Premier client : le navigateur.

## Installation, mise à jour et gestion

Une seule commande, en root, pour tout : installer, mettre à jour, sauvegarder,
restaurer, désinstaller. Elle ouvre un menu.

```bash
bash <(curl -fsSL https://raw.githubusercontent.com/Victor-root/melyxar/develop/scripts/melyxar.sh)
```

La même commande sert à mettre à jour. Le script est lu depuis le dépôt à
chaque lancement, donc c'est toujours la version publiée qui tourne : une copie
gardée sur la machine cesse d'être à jour le jour où le script change, et rien
ne le signale.

Pour une action directe, sans passer par le menu, ajoutez son nom :

```bash
bash <(curl -fsSL https://raw.githubusercontent.com/Victor-root/melyxar/develop/scripts/melyxar.sh) update
```

Les actions sont `install`, `update`, `status`, `backup`, `restore` et
`uninstall`. C'est cette forme qu'attend une tâche planifiée, puisqu'elle ne
pose aucune question.

### Quand quelque chose ne va pas

Tant que Melyxar est en construction, le script montre tout : chaque commande
et sa sortie au fur et à mesure, la compilation comprise. C'est ce qu'il faut
joindre à un rapport de problème, parce qu'une étape qui a réussi peut très
bien avoir dit quelque chose d'utile en chemin.

Pour retrouver l'affichage sobre, une ligne par étape et la sortie complète
seulement de ce qui échoue :

```bash
bash <(curl -fsSL https://raw.githubusercontent.com/Victor-root/melyxar/develop/scripts/melyxar.sh) update --quiet
```

`MELYXAR_VERBOSE=0` dans l'environnement fait la même chose, une fois pour
toutes. Ce sera le défaut le jour où le serveur sera fini.

## Où vivent les choses

| | |
|---|---|
| Configuration | `/etc/melyxar/melyxar.toml` |
| Données et sauvegardes | `/var/lib/melyxar` |
| Cache | `/var/cache/melyxar` |
| Sources | `/opt/melyxar/source` |

## Documentation

Tout est dans `docs/architecture/` :

- `README.md` : les décisions actées, et pourquoi.
- `01-revue-architecture.md` : l'architecture complète.
- `02-reactivite.md` : l'exigence de réactivité et ses budgets.
- `03-plan.md` : le plan par jalons et où il en est.
