# Logo

`melyxar-logo.png` est l'original, et rien d'autre ne doit être considéré
comme tel : 4293 x 4514, fond transparent, sans marge. Il n'est jamais servi
au navigateur. Ce qui est servi vit dans `web/public/` et en est dérivé.

## Ce qui en est tiré

| Fichier | Taille | Où |
|---|---|---|
| `web/public/melyxar-512.png` | 512 | Le logo de l'application, pour tout usage général |
| `web/public/melyxar-64.png` | 64 | La marque de l'en-tête, affichée à 26 pixels |
| `web/public/apple-touch-icon.png` | 180 | Écran d'accueil sur téléphone et tablette |
| `web/public/favicon-32.png` | 32 | Onglet du navigateur, écran dense |
| `web/public/favicon-16.png` | 16 | Onglet du navigateur, écran ordinaire |

Le dessin est recadré sur ce qui se voit vraiment, puis posé au centre d'un
carré. Marge de 5 % pour le logo, 1 % pour les icônes : une icône d'onglet est
vue seule et jamais à côté des autres tailles, et chaque pixel compte quand il
n'y en a que seize.

## Pourquoi des images fixes et pas un SVG

Les tailles auxquelles ce logo est affiché sont connues d'avance et il n'y en
a que cinq, donc les tailler une fois vaut mieux que les recalculer à chaque
affichage. C'est la même règle que pour les affiches des films, décidée dans
`docs/architecture/README.md`.

## Pour en refaire un jeu

Remplacer l'original et régénérer les cinq tailles avec n'importe quel outil
d'image, en gardant les marges ci-dessus. Aucune étape n'est automatisée : un
logo change une fois tous les deux ans, et un script qu'on lance deux fois est
un script que personne ne sait plus lancer.
