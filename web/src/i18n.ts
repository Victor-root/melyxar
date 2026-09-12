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
  "search.results": "{count} found",
  "search.nothing": "Nothing found.",

  "work.play": "Play",
  "work.trailer": "Trailer",
  "work.more": "More",
  "work.less": "Less",
  "work.cast": "Cast",
  "work.crew": "Crew",
  "work.versions": "Versions",
  "work.version": "Version",
  "work.details": "File details",
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
  "work.back": "Back",

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
  "error.generic": "Something went wrong.",
  "error.retry": "Try again",

  "credit.actor": "Actor",
  "credit.director": "Director",
  "credit.writer": "Writer",
  "credit.producer": "Producer",
  "credit.composer": "Composer",

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
  "search.results": "{count} trouvés",
  "search.nothing": "Rien trouvé.",

  "work.play": "Lire",
  "work.trailer": "Bande annonce",
  "work.more": "Plus",
  "work.less": "Moins",
  "work.cast": "Distribution",
  "work.crew": "Équipe",
  "work.versions": "Versions",
  "work.version": "Version",
  "work.details": "Détails du fichier",
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
  "work.back": "Retour",

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
  "error.generic": "Quelque chose s'est mal passé.",
  "error.retry": "Réessayer",

  "credit.actor": "Acteur",
  "credit.director": "Réalisation",
  "credit.writer": "Scénario",
  "credit.producer": "Production",
  "credit.composer": "Musique",

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
