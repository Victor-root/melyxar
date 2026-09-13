/*
 * Every word the interface says lives here, English first and French second.
 * Nothing anywhere else holds a sentence: that is what makes adding a third
 * language a matter of adding a column rather than of hunting through screens.
 */

export type Language = "en" | "fr";

type Dictionary = Record<string, string>;

const en: Dictionary = {
  "app.name": "Melyxar",
  "nav.home": "Home",
  "nav.libraries": "Libraries",
  "nav.search": "Search",
  "nav.jobs": "Activity",
  "nav.theme": "Theme",
  "nav.language": "Language",

  "theme.dark": "Dark",
  "theme.light": "Light",
  "theme.system": "Follow the system",

  "home.recently_added": "Recently added",
  "home.empty.title": "Nothing here yet",
  "home.empty.body": "Point a library at a folder and run a scan to fill this page.",
  "home.awaiting": "{count} still waiting to be looked up",
  "home.scan": "Scan",
  "home.identify": "Look up what is missing",

  "library.all": "All",
  "library.count": "{count} works",
  "library.sort": "Sort",
  "library.sort.title": "Title",
  "library.sort.added_at": "Date added",
  "library.sort.release_year": "Year",
  "library.sort.community_rating": "Rating",
  "library.sort.runtime": "Length",
  "library.descending": "Reverse order",
  "library.filter.genre": "Genre",
  "library.filter.decade": "Decade",
  "library.filter.any": "Any",
  "library.filter.unidentified": "Not identified",
  "library.empty": "Nothing matches.",
  "library.loading": "Loading",
  "library.end": "That is everything.",

  "search.placeholder": "Search a title",

  "work.play": "Play",
  "work.trailer": "Trailer",
  "work.more": "More",
  "work.less": "Less",
  "work.cast": "Cast",
  "work.crew": "Crew",
  "work.versions": "Versions",
  "work.version": "Version",
  "work.collection": "Part of {name}",
  "work.minutes": "{count} min",
  "work.unidentified": "Not identified",
  "work.pending": "Waiting to be looked up",
  "work.missing": "The file is not on the disk",
  "work.not_analysed": "Not analysed yet",
  "work.audio": "Audio",
  "work.subtitles": "Subtitles",
  "work.video": "Picture",
  "work.chapters": "{count} chapters",
  "work.external": "Separate file",
  "work.forced": "Forced",
  "work.hearing_impaired": "For the hard of hearing",
  "work.burns_in": "Has to be burnt into the picture",
  "work.no_overview": "No synopsis yet.",

  "jobs.title": "Activity",
  "jobs.running": "Running",
  "jobs.recent": "Finished",
  "jobs.none": "Nothing is running.",
  "jobs.cancel": "Stop",
  "jobs.scan_library": "Scan",
  "jobs.identify_work": "Identification",
  "jobs.fetch_images": "Pictures",
  "jobs.analyse_loudness": "Loudness",
  "jobs.generate_thumbnails": "Thumbnails",
  "jobs.purge_activity": "Tidying",
  "jobs.backup": "Backup",
  "jobs.state.queued": "Waiting",
  "jobs.state.running": "Running",
  "jobs.state.succeeded": "Done",
  "jobs.state.failed": "Failed",
  "jobs.state.cancelled": "Stopped",

  "root.root_missing_or_not_mounted": "Not found. The disk is probably not mounted.",
  "root.root_not_readable_by_server_user": "Present, but the server cannot read it.",
  "root.root_readable_only": "Readable.",
  "root.root_readable_and_writable": "Readable and writable.",

  "error.unreachable": "The server did not answer.",
  "error.not_found": "Nothing there.",
  "error.retry": "Try again",

  "credit.actor": "Actor",
  "credit.director": "Director",
  "credit.writer": "Writer",
  "credit.producer": "Producer",
  "credit.composer": "Composer",

  "player.close": "Back",
  "player.rebuilt": "Prepared by the server",
  "player.resume": "Resume",
  "player.from_the_start": "From the beginning",
  "player.no_subtitle": "None",
  "player.speed": "Speed",
  "player.corner": "Corner",
  "player.unknown_language": "Language not stated",
  "player.burns_in_short": "drawn into the picture",
  "player.step.starting": "Getting ready...",
  "player.step.reading": "Reading the film...",
  "player.step.producing": "Preparing the first few seconds...",
  "player.step.ready": "Starting...",
  "player.segments_ready": "{ready} of {wanted}",

  "player.subtitle_size": "Subtitle size",
  "player.subtitle_size.small": "Small",
  "player.subtitle_size.normal": "Normal",
  "player.subtitle_size.large": "Large",
  "player.subtitle_size.huge": "Very large",
  "player.subtitle_colour": "Subtitle colour",
  "player.subtitle_colour.white": "White",
  "player.subtitle_colour.yellow": "Yellow",
  "player.subtitle_colour.cyan": "Blue",
  "player.subtitle_edge": "Readability",
  "player.subtitle_edge.outline": "Outline",
  "player.subtitle_edge.shadow": "Shadow",
  "player.subtitle_edge.none": "Nothing",
  "player.subtitle_background": "Behind the words",
  "player.subtitle_background.none": "Nothing",
  "player.subtitle_background.dim": "Darkened",
  "player.subtitle_background.solid": "Solid",
  "player.subtitle_height": "Height",
  "player.subtitle_height.bottom": "At the bottom",
  "player.subtitle_height.raised": "A little higher",
  "player.subtitle_height.high": "Higher still",

  "player.missing": "The file is not on the disk at the moment.",
  "player.cannot_play": "This browser could not play the film.",
  "player.too_busy": "The server is already rebuilding as many films as it can at once. Try again in a moment.",
  "player.no_conversion": "This server has no media tools, so a film it cannot hand over as it is cannot be played.",

  "playback.direct_play": "Played as it is on the disk.",
  "playback.remux": "Repackaged on the way out, without touching the picture or the sound.",
  "playback.transcode_audio": "The sound is rebuilt, the picture is untouched.",
  "playback.full_transcode": "The picture and the sound are both rebuilt.",

  "reason.everything_supported": "this browser opens the file as it is",
  "reason.container_not_supported": "the container is not one this browser opens",
  "reason.video_codec_not_supported": "the picture is in a form this browser cannot decode",
  "reason.video_profile_not_supported": "the picture is in a variant this browser cannot decode",
  "reason.resolution_too_high": "the picture is larger than this browser accepts",
  "reason.wide_gamut_not_supported": "the colours have to be converted",
  "reason.dolby_vision_without_base_layer": "this colour format cannot be shown as it is",
  "reason.interlaced_picture": "the picture is interlaced",
  "reason.bitrate_too_high": "the film arrives faster than this connection accepts",
  "reason.audio_codec_not_supported": "the sound is in a form this browser cannot decode",
  "reason.too_many_audio_channels": "the sound has more channels than this browser accepts",
  "reason.downmix_requested": "a fold to stereo was asked for",
  "reason.loudness_levelling_requested": "levelling the loudness was asked for",
  "reason.subtitle_must_be_burned_in": "these subtitles can only be drawn into the picture",
  "reason.subtitle_format_not_drawn_by_client": "this browser draws no subtitle this server can send alongside",
  "reason.non_default_track_selected": "a track other than the default one was chosen",

  "attribution.tmdb":
    "This product uses the TMDB API but is not endorsed or certified by TMDB.",
};

const fr: Dictionary = {
  "app.name": "Melyxar",
  "nav.home": "Accueil",
  "nav.libraries": "Bibliothèques",
  "nav.search": "Rechercher",
  "nav.jobs": "Activité",
  "nav.theme": "Thème",
  "nav.language": "Langue",

  "theme.dark": "Sombre",
  "theme.light": "Clair",
  "theme.system": "Suivre le système",

  "home.recently_added": "Récemment ajoutés",
  "home.empty.title": "Rien pour l'instant",
  "home.empty.body":
    "Indiquez un dossier à une bibliothèque et lancez un scan pour remplir cette page.",
  "home.awaiting": "{count} en attente d'identification",
  "home.scan": "Scanner",
  "home.identify": "Identifier ce qui manque",

  "library.all": "Tout",
  "library.count": "{count} œuvres",
  "library.sort": "Trier",
  "library.sort.title": "Titre",
  "library.sort.added_at": "Date d'ajout",
  "library.sort.release_year": "Année",
  "library.sort.community_rating": "Note",
  "library.sort.runtime": "Durée",
  "library.descending": "Ordre inverse",
  "library.filter.genre": "Genre",
  "library.filter.decade": "Décennie",
  "library.filter.any": "Tous",
  "library.filter.unidentified": "Non identifiés",
  "library.empty": "Rien ne correspond.",
  "library.loading": "Chargement",
  "library.end": "C'est tout.",

  "search.placeholder": "Chercher un titre",

  "work.play": "Lire",
  "work.trailer": "Bande annonce",
  "work.more": "Plus",
  "work.less": "Moins",
  "work.cast": "Distribution",
  "work.crew": "Équipe",
  "work.versions": "Versions",
  "work.version": "Version",
  "work.collection": "Fait partie de {name}",
  "work.minutes": "{count} min",
  "work.unidentified": "Non identifié",
  "work.pending": "En attente d'identification",
  "work.missing": "Le fichier n'est pas sur le disque",
  "work.not_analysed": "Pas encore analysé",
  "work.audio": "Audio",
  "work.subtitles": "Sous-titres",
  "work.video": "Image",
  "work.chapters": "{count} chapitres",
  "work.external": "Fichier séparé",
  "work.forced": "Forcés",
  "work.hearing_impaired": "Malentendants",
  "work.burns_in": "Doit être incrusté dans l'image",
  "work.no_overview": "Pas encore de synopsis.",

  "jobs.title": "Activité",
  "jobs.running": "En cours",
  "jobs.recent": "Terminées",
  "jobs.none": "Rien ne tourne.",
  "jobs.cancel": "Arrêter",
  "jobs.scan_library": "Scan",
  "jobs.identify_work": "Identification",
  "jobs.fetch_images": "Images",
  "jobs.analyse_loudness": "Sonie",
  "jobs.generate_thumbnails": "Vignettes",
  "jobs.purge_activity": "Rangement",
  "jobs.backup": "Sauvegarde",
  "jobs.state.queued": "En attente",
  "jobs.state.running": "En cours",
  "jobs.state.succeeded": "Terminée",
  "jobs.state.failed": "Échouée",
  "jobs.state.cancelled": "Arrêtée",

  "root.root_missing_or_not_mounted": "Introuvable. Le disque n'est probablement pas monté.",
  "root.root_not_readable_by_server_user": "Présent, mais le serveur ne peut pas le lire.",
  "root.root_readable_only": "Lisible.",
  "root.root_readable_and_writable": "Lisible et inscriptible.",

  "error.unreachable": "Le serveur n'a pas répondu.",
  "error.not_found": "Il n'y a rien ici.",
  "error.retry": "Réessayer",

  "credit.actor": "Acteur",
  "credit.director": "Réalisation",
  "credit.writer": "Scénario",
  "credit.producer": "Production",
  "credit.composer": "Musique",

  "player.close": "Retour",
  "player.rebuilt": "Préparé par le serveur",
  "player.resume": "Reprendre",
  "player.from_the_start": "Depuis le début",
  "player.no_subtitle": "Aucun",
  "player.speed": "Vitesse",
  "player.corner": "Dans un coin",
  "player.unknown_language": "Langue non précisée",
  "player.burns_in_short": "incrustés dans l’image",
  "player.step.starting": "Préparation...",
  "player.step.reading": "Lecture du film...",
  "player.step.producing": "Préparation des premières secondes...",
  "player.step.ready": "Démarrage...",
  "player.segments_ready": "{ready} sur {wanted}",

  "player.subtitle_size": "Taille des sous-titres",
  "player.subtitle_size.small": "Petite",
  "player.subtitle_size.normal": "Normale",
  "player.subtitle_size.large": "Grande",
  "player.subtitle_size.huge": "Très grande",
  "player.subtitle_colour": "Couleur des sous-titres",
  "player.subtitle_colour.white": "Blanc",
  "player.subtitle_colour.yellow": "Jaune",
  "player.subtitle_colour.cyan": "Bleu",
  "player.subtitle_edge": "Lisibilité",
  "player.subtitle_edge.outline": "Contour",
  "player.subtitle_edge.shadow": "Ombre",
  "player.subtitle_edge.none": "Rien",
  "player.subtitle_background": "Derrière les mots",
  "player.subtitle_background.none": "Rien",
  "player.subtitle_background.dim": "Assombri",
  "player.subtitle_background.solid": "Plein",
  "player.subtitle_height": "Hauteur",
  "player.subtitle_height.bottom": "En bas",
  "player.subtitle_height.raised": "Un peu plus haut",
  "player.subtitle_height.high": "Encore plus haut",

  "player.missing": "Le fichier n'est pas sur le disque en ce moment.",
  "player.cannot_play": "Ce navigateur n'a pas réussi à lire le film.",
  "player.too_busy": "Le serveur reconstruit déjà autant de films qu'il peut à la fois. Réessayez dans un instant.",
  "player.no_conversion": "Ce serveur n'a pas les outils multimédias : un film qu'il ne peut pas transmettre tel quel ne peut pas être lu.",

  "playback.direct_play": "Lu tel quel depuis le disque.",
  "playback.remux": "Remis dans une autre boîte en sortant, sans toucher à l'image ni au son.",
  "playback.transcode_audio": "Le son est reconstruit, l'image reste intacte.",
  "playback.full_transcode": "L'image et le son sont tous les deux reconstruits.",

  "reason.everything_supported": "ce navigateur ouvre le fichier tel quel",
  "reason.container_not_supported": "la boîte n'est pas de celles que ce navigateur ouvre",
  "reason.video_codec_not_supported": "l'image est dans une forme que ce navigateur ne sait pas décoder",
  "reason.video_profile_not_supported": "l'image est dans une variante que ce navigateur ne sait pas décoder",
  "reason.resolution_too_high": "l'image est plus grande que ce que ce navigateur accepte",
  "reason.wide_gamut_not_supported": "les couleurs doivent être converties",
  "reason.dolby_vision_without_base_layer": "ce format de couleurs ne peut pas être montré tel quel",
  "reason.interlaced_picture": "l'image est entrelacée",
  "reason.bitrate_too_high": "le film arrive plus vite que ce que cette connexion accepte",
  "reason.audio_codec_not_supported": "le son est dans une forme que ce navigateur ne sait pas décoder",
  "reason.too_many_audio_channels": "le son a plus de canaux que ce que ce navigateur accepte",
  "reason.downmix_requested": "un repliement en stéréo a été demandé",
  "reason.loudness_levelling_requested": "le nivellement du volume a été demandé",
  "reason.subtitle_must_be_burned_in": "ces sous-titres ne peuvent être qu'incrustés dans l'image",
  "reason.subtitle_format_not_drawn_by_client": "ce navigateur n'affiche aucun sous-titre que ce serveur sait envoyer à côté",
  "reason.non_default_track_selected": "une piste autre que celle par défaut a été choisie",

  "attribution.tmdb":
    "Ce produit utilise l'API de TMDB mais n'est ni approuvé ni certifié par TMDB.",
};

const dictionaries: Record<Language, Dictionary> = { en, fr };

const STORED_LANGUAGE = "melyxar.language";

/** The language to speak: what was chosen, otherwise what the browser asks for. */
export function initialLanguage(): Language {
  const stored = safeRead(STORED_LANGUAGE);
  if (stored === "en" || stored === "fr") {
    return stored;
  }
  return navigator.language.toLowerCase().startsWith("fr") ? "fr" : "en";
}

export function rememberLanguage(language: Language): void {
  safeWrite(STORED_LANGUAGE, language);
}

/**
 * The wording of one key.
 *
 * A key nobody translated comes back as itself rather than as an empty space,
 * so a missing word is visible during development instead of silently blank.
 */
export function translate(
  language: Language,
  key: string,
  values?: Record<string, string | number>,
): string {
  const wording = dictionaries[language][key] ?? dictionaries.en[key] ?? key;
  if (!values) {
    return wording;
  }
  return Object.entries(values).reduce(
    (text, [name, value]) => text.replaceAll(`{${name}}`, String(value)),
    wording,
  );
}

/** Reading and writing what the browser remembers, which can be forbidden. */
export function safeRead(key: string): string | null {
  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
}

export function safeWrite(key: string, value: string): void {
  try {
    window.localStorage.setItem(key, value);
  } catch {
    // A browser that refuses to remember is a browser that asks again next
    // time, which is a small annoyance and never a failure.
  }
}
