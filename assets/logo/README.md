# Logo

`melyxar-logo.png` est l'original, et rien d'autre ne doit être considéré
comme tel : 4293 x 4514, fond transparent, sans marge. Il n'est jamais servi
au navigateur. Ce qui est servi vit dans `web/public/` et en est dérivé.

## Ce qui en est tiré

| Fichier | Taille | Où |
|---|---|---|
| `web/public/melyxar-1024.webp` | 1024 | L'écran de démarrage, où le logo est affiché grand |
| `web/public/melyxar-shade-512.png` | 512 | Le relief du logo, pour tout usage au-delà de 64 pixels (écran de connexion) |
| `web/public/melyxar-shade-64.png` | 64 | Le relief du logo, pour la barre du haut et l'administration |
| `web/public/apple-touch-icon.png` | 180 | Écran d'accueil sur téléphone et tablette |
| `web/public/favicon-32.png` | 32 | Onglet du navigateur, écran dense |
| `web/public/favicon-16.png` | 16 | Onglet du navigateur, écran ordinaire |

Le dessin est recadré sur ce qui se voit vraiment, puis posé au centre d'un
carré. Marge de 5 % pour le logo, 1 % pour les icônes : une icône d'onglet est
vue seule et jamais à côté des autres tailles, et chaque pixel compte quand il
n'y en a que seize.

## La taille de l'écran de démarrage

Recadré et centré, l'original donne un carré de 5006 pixels de côté, donc la
netteté n'est jamais la contrainte : c'est le poids du fichier qui décide.

La règle est de servir deux fois la taille d'affichage, parce qu'un écran
dense met deux pixels là où la page en demande un. Les 1024 pixels couvrent
donc un logo affiché jusqu'à 512 pixels, ce qui est déjà un grand écran de
démarrage. Au-delà, refaire une taille depuis l'original : 1536 pèse 144 Ko et
2048 pèse 221 Ko, contre 83 Ko ici.

En WebP et non en PNG parce qu'à cette taille le PNG pèse 427 Ko pour un
écart invisible : une fois le logo posé sur un fond, la différence dépasse
huit niveaux sur deux millièmes des pixels, et jamais sur le contour, que
WebP restitue à l'identique.

## Le logo dans la couleur d'accentuation

Dans l'interface, le logo prend la couleur d'accentuation choisie par chaque
compte. Ce qui est servi n'est donc pas le logo rouge mais son relief : une
image en niveaux de gris qui ne garde que la lumière et l'ombre du ruban, avec
la transparence d'origine. L'interface pose une couleur vive tirée de
l'accentuation (sa teinte, sa saturation poussée de 35 %, une luminosité de
45 %), la découpe à la forme du logo, et applique le relief par-dessus en
lumière dure : un gris moyen laisse la couleur telle quelle, plus clair
l'éclaircit, plus sombre l'assombrit.

Le relief est calculé pour rendre le logo d'origine le plus fidèlement
possible avec l'accentuation par défaut, qui donne le rouge pur à 45 % de
luminosité : pour chaque pixel de la version rouge recadrée, on garde le gris
dont le résultat en lumière dure sur ce rouge est le plus proche du pixel
d'origine (écart pondéré rouge 0,5, vert 1, bleu 0,6), transparence
inchangée. L'écart moyen restant est d'environ douze niveaux sur 255,
invisible à l'œil. Les versions rouges dont il part se refont depuis
l'original, recadrées et marginées comme les autres tailles.

L'icône de l'onglet suit aussi : dès qu'un compte a choisi une autre couleur,
la page la redessine de la même façon à partir du relief à 64 pixels et la met
à la place des icônes rouges, qui restent celles du premier affichage et de
la couleur par défaut. L'icône d'écran d'accueil garde le rouge : le téléphone
la copie une fois pour toutes au moment où on l'épingle.

## Pourquoi des images fixes et pas un SVG

Les tailles auxquelles ce logo est affiché sont connues d'avance et il n'y en
a que six, donc les tailler une fois vaut mieux que les recalculer à chaque
affichage. C'est la même règle que pour les affiches des films, décidée dans
`docs/architecture/README.md`.

## Pour en refaire un jeu

Remplacer l'original et régénérer les six fichiers ci-dessus (les deux reliefs se tirent de la version rouge à 512 et à 64 pixels, recadrée et marginée comme les autres) avec n'importe quel outil
d'image, en gardant les marges ci-dessus. Aucune étape n'est automatisée : un
logo change une fois tous les deux ans, et un script qu'on lance deux fois est
un script que personne ne sait plus lancer.
