# Films d'essai

Une médiathèque faite pour malmener Melyxar : chaque film y est choisi pour un cas difficile. On la remplit avec l'action « Films d'essai » du script d'installation (`melyxar.sh test-films`), puis on l'ajoute comme bibliothèque de films dans Melyxar. Environ 7 Go, et une vingtaine de minutes de fabrication la première fois.

Tout y est libre de droits :
- les films d'essai de Jellyfin (Creative Commons BY-SA, par Gnattu, sur repo.jellyfin.org/test-videos) ;
- les films libres de la Blender Foundation (Creative Commons BY, sur download.blender.org) ;
- ce qui est fabriqué à partir d'eux sur place par notre FFmpeg.

| Fichier | Rôle |
|---|---|
| `downloads` | Les films téléchargés tels quels, chacun vérifié par son empreinte SHA-256. |
| `build.sh` | Télécharge, vérifie, puis fabrique les cas qu'aucun film libre ne contient. Reprend là où il s'est arrêté. |
| `pgs.py` | Écrit des sous-titres en images (format des Blu-ray), que FFmpeg sait lire mais pas écrire. |
| `styled.ass` | Des sous-titres stylés écrits pour l'occasion : chaque réplique teste une chose. |

## Ce que chaque film teste et ce qu'on doit voir

Pour chacun : le lancer tel quel, puis en forçant une conversion (qualité réduite), avancer au milieu, changer de piste audio et de sous-titres.

**L'image**

| Film | Teste | On doit voir |
|---|---|---|
| Melyxar test - 1080p H.264 | Le cas simple | Lu directement, sans conversion. |
| Melyxar test - 4K HEVC 10-bit | Le HEVC 10 bits sans HDR | Couleurs normales, aucune conversion des couleurs. |
| Melyxar test - 4K AV1 10-bit | L'AV1, que toutes les cartes ne savent pas écrire | Lu ou converti sans erreur. |
| Melyxar test - 1080p HDR10 HEVC | Le HDR en petite taille | Converti : couleurs naturelles, ni grises ni délavées. |
| Melyxar test - 4K HDR10 HEVC | Le HDR en 4K | Pareil, et fluide. |
| Melyxar test - 4K HDR10 AV1 | Le HDR en AV1 | Pareil. |
| Melyxar test - 8K HDR10 HEVC | La plus grosse image | La carte tient, ou passe la main proprement. |
| Melyxar test - 4K Dolby Vision profile 5 | Le Dolby Vision « seul », sans HDR10 dessous | Pas d'image verte et violette. |
| Melyxar test - 4K Dolby Vision profile 8.1 | Le Dolby Vision avec HDR10 dessous | Couleurs naturelles. |
| Melyxar test - 4K Dolby Vision profile 8.4 | Le Dolby Vision avec HLG dessous | Couleurs naturelles. |
| Big Buck Bunny (2008) | La 4K à 60 images par seconde | Fluide, sans saccade. |
| Melyxar test - 25 fps / 29.97 fps | Les cadences de la télévision | Fluide, son calé. |
| Melyxar test - variable frame rate | Une cadence qui change en cours de route (comme un téléphone) | Le son reste calé sur l'image. |
| Melyxar test - interlaced 1080i MPEG-2 | L'entrelacé de la télévision | Pas de peignes sur les mouvements. |
| Melyxar test - DVD 576i 4-3 | Un DVD en 4/3, bandes noires dans l'image | Format 4/3, pas étiré, pas de peignes. |
| Melyxar test - black bars in the picture | Des bandes noires écrites dans l'image | L'image tient dans l'écran. |
| Melyxar test - 4-3 | Une image 4/3 | Bandes noires sur les côtés, pas étirée. |

**Le son**

| Film | Teste | On doit voir |
|---|---|---|
| Sintel (2010) | Le son AC3 5.1 | Son présent, voix au centre audibles en stéréo. |
| Melyxar test - TrueHD 5.1 | Un son de Blu-ray que les navigateurs ne lisent pas | Converti, son présent et calé. |
| Melyxar test - DTS 5.1 | Pareil, en DTS | Pareil. |
| Melyxar test - several sound tracks | Trois pistes : originale, française, commentaire | On change de piste sans casse. |
| Tears of Steel (2012) | Un fichier MOV | Lu normalement. |

**Les sous-titres**

| Film | Teste | On doit voir |
|---|---|---|
| Sintel (2010) | Dix pistes de sous-titres texte | Toutes proposées, dans la bonne langue. |
| Melyxar test - 4K HDR10 with picture subtitles | Des sous-titres en images sur un film HDR 4K | Couleurs justes, sous-titres incrustés en bas, un sur trois en haut, accents lisibles. |
| Melyxar test - 1080p HDR10 HEVC (fichier .fr.sup) | Des sous-titres en images posés à côté du film | Proposés en français, incrustés. |
| Melyxar test - styled subtitles | Des sous-titres stylés | Couleurs, positions, rotation, fondus, karaoké respectés. |
| Melyxar test - forced subtitles | Des sous-titres forcés, dedans et à côté | Repérés comme forcés. |
| Melyxar test - external subtitles | Des sous-titres à côté du film, dont un en ancien encodage | Accents français corrects. |

**La structure du fichier**

| Film | Teste | On doit voir |
|---|---|---|
| Melyxar test - chapters | Des chapitres toutes les 30 secondes | Chapitres proposés, démarrage au milieu d'un chapitre sans souci. |
| Melyxar test - old AVI | Un vieux format (MPEG-4 et MP3 dans un AVI) | Lu ou converti sans erreur. |
| Melyxar test - cut off partway | Un fichier coupé, comme un téléchargement interrompu | Joue jusqu'où il peut, sans planter. |
| Melyxar test - three hours | Un film très long | On avance n'importe où, la reprise de lecture tient. |
