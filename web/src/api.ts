/*
 * The one place that talks to the server.
 *
 * Every shape here mirrors what the server sends, so a change on that side
 * fails at compilation rather than at three in the morning on a screen.
 */

/** An account, as the interface is told about it. */
export interface Account {
  id: string;
  name: string;
  /** What the interface hides things by. The server refuses them too: this is
      what somebody is shown, never what they are allowed. */
  is_administrator: boolean;
  may_download: boolean;
  may_manage_collections: boolean;
  may_delete: boolean;
  may_delete_from_disk: boolean;
  /** Whether it may write into the files of songs with the tag manager. */
  may_edit_tags: boolean;
  /** Whether it may put files into the libraries. */
  may_upload: boolean;
  /** Where the picture it chose is served, when it chose one. */
  avatar: string | null;
}

/** What an account may do and see, as the administration shows and sends
 *  it. An administrator holds every right whatever is sent. */
export interface Rights {
  is_administrator: boolean;
  sees_every_library: boolean;
  /** The libraries granted, read only when it does not see every one. */
  libraries: string[];
  may_delete: boolean;
  may_delete_from_disk: boolean;
  /** Whether it may keep a copy of a file on its own device. */
  may_download: boolean;
  /** Whether it may make and fill the server's collections. */
  may_manage_collections: boolean;
  /** Whether it may write into the files of songs with the tag manager. */
  may_edit_tags: boolean;
  /** Whether it may put files into the libraries. */
  may_upload: boolean;
  /** How many films it may watch at once. Nothing for no limit. */
  most_streams: number | null;
}

/** One account as the page of accounts lists it. */
export interface ManagedAccount {
  id: string;
  name: string;
  avatar: string | null;
  /** Whether this is the administrator looking at the page. */
  is_you: boolean;
  rights: Rights;
  created_at: string;
  /** How many devices it is signed in on. */
  devices: number;
  /** When the most recent of them was last used. */
  last_seen_at: string | null;
}

/** One device signed in to this server, as a list of them shows it. */
export interface SignedInDevice {
  id: string;
  user_id: string;
  user_name: string;
  user_avatar: string | null;
  /** What the browser said it was when it signed in. */
  name: string;
  /** The browser its page found, when it could tell better than the line. */
  browser: string | null;
  /** False for a session that ends when the browser is closed. */
  remembered: boolean;
  signed_in_at: string;
  /** Up to an hour behind the real last use. */
  last_seen_at: string;
  /** Whether this is the device the list is being looked at from. */
  is_this_one: boolean;
}

/** A name the sign in screen offers, with its picture when it has one. */
export interface NameAtTheDoor {
  name: string;
  avatar: string | null;
}

/**
 * What the door needs to draw itself, before anybody has signed in.
 *
 * The one thing this server says to somebody it does not know: its name, its
 * mark, and whether it has been set up at all. No account list, no library
 * name, no version.
 */
/** What is drawn behind the sign in screen when no picture was put there.
 *
 *  Read as a word rather than matched on, so that a server newer than this
 *  interface naming a third one is drawn with the usual rather than with
 *  nothing at all. */
export type LoginBackgroundStyle = "abstract" | "library" | "picture";

/** What this server calls itself and wears, before anybody has signed in.
 *
 *  Everything here belongs to the administrator rather than to Melyxar: what
 *  the server ships with is a fallback the screen holds, never something the
 *  screen assumes. */
/** What the server is called and where its logo is, when it was given one. */
export type ServerIdentity = Pick<Branding, "server_name" | "logo" | "logo_icon" | "default_theme">;

/** A theme the administrator can make the server's own. */
export type ServerTheme = "dark" | "light" | "system";

/** The same, as the administrator changing them reads it: with the name the
 *  server would be given back. */
export interface ServerSettings extends ServerIdentity {
  default_name: string;
  /** Which background the sign in screen wears. */
  door_background: LoginBackgroundStyle;
  /** Where the picture sent for it is, when there is one, kept even while
   *  another background is chosen. */
  door_picture: string | null;
  /** The line under the server's name, or nothing for Melyxar's own. */
  door_slogan: string | null;
  /** The longest it may be, in letters. */
  longest_slogan: number;
}

export interface Branding {
  server_name: string;
  /** Where the logo the administrator gave the server is, or nothing for
      Melyxar's own. */
  logo: string | null;
  /** The same logo as a square icon, for the tab of the browser. */
  logo_icon: string | null;
  /** The picture behind the sign in screen, while it is the chosen
   *  background. */
  login_background_path: string | null;
  login_background_style: LoginBackgroundStyle;
  /** The line under the server's name as the administrator wrote it, or
   *  nothing for Melyxar's own, worded in the language of whoever looks. */
  door_slogan: string | null;
  /** The theme of whoever has not chosen one. */
  default_theme: ServerTheme;
  /** False on a brand new server, which asks for a first account instead of a
      password. */
  setup_complete: boolean;
}

export interface Picture {
  url: string;
  width: number | null;
  height: number | null;
}

export type WorkKind = "movie" | "series" | "season" | "episode" | "folder" | "video" | "photo";

/** What a library holds, which is what the header's categories are built
    from: a kind with no library of it is a category that does not exist. */
export type LibraryKind = "movies" | "series" | "anime" | "shows" | "home_media" | "music";

export type Seen = "not_started" | "in_progress" | "watched";

export interface Card {
  id: string;
  title: string;
  /** The letter it is filed under in a grid read by title, "#" for none. */
  initial: string;
  year: number | null;
  runtime_minutes: number | null;
  rating: number | null;
  /* "own" is what somebody filmed or photographed themselves: in no
     catalogue, and waiting for none. */
  identification: "pending" | "identified" | "unidentified" | "manual" | "own";
  /* Why the last look up failed, when one has run. Absent for a film nobody
     has looked up yet, which is itself the answer. */
  identification_note: IdentificationNote | null;
  color: string | null;
  poster: Picture[];
  /** A picture wider than it is tall, for the rows that lie a card down. A
      poster cropped into a band is a poster beheaded. Empty elsewhere, and
      empty where there is nothing wide, which such a row answers by falling
      back to the poster. */
  wide: Picture[];
  /** What it is: a film plays from its card, a series is opened. */
  kind: WorkKind;
  library: string;
  /* What the account asking has made of it. Every one of these is what the
     hover shows, which is why they travel with the card rather than being
     asked for one card at a time. */
  seen: Seen;
  /** Whether its library keeps watched marks: where it does not, the card is
      never marked and offers no button to mark it. */
  watched_marks: boolean;
  /** Where they stopped, in seconds, only where they stopped partway. */
  resume_from_seconds: number | null;
  /** How long the copy it is carried on in lasts, in seconds, beside where
   *  it stopped. */
  resume_length_seconds: number | null;
  favourite: boolean;
  /** Whether this account put it aside to watch later. */
  watch_later: boolean;
  /** Episodes below, and how many are left. Both nothing for a film. */
  episodes: number;
  unwatched: number;
  /** The copy a play button on the card starts, when one is on disk. */
  source: string | null;
}

/** A film a person could have meant, as the provider describes it. */
/** What a work says about itself, as an administrator writes it by hand. */
export interface WrittenDetails {
  title: string;
  tagline: string | null;
  overview: string | null;
  release_year: number | null;
  /** The day it came out, and for a series the day it ended, year, month,
   *  day. */
  release_date: string | null;
  end_date: string | null;
  community_rating: number | null;
  age_rating: string | null;
  genres: string[];
  studios: string[];
  /** Everybody credited, in the order the page shows them. */
  credits: WrittenCredit[];
  /** The fields no look up changes any more, by the name of the field. */
  locked: DetailField[];
  /** The roles somebody may be credited in, as the server says them. */
  roles: string[];
}

/** One person credited on a work, and who they play when they act. */
export interface WrittenCredit {
  name: string;
  role: string;
  character: string | null;
}

/** A field of a work that may be written by hand, and locked. */
export type DetailField = Exclude<keyof WrittenDetails, "locked" | "roles">;

export interface Candidate {
  external_id: string;
  title: string;
  original_title: string | null;
  year: number | null;
  overview: string | null;
  poster: string | null;
}

/**
 * What somebody typed to find the right work.
 *
 * All of it as written, strings included for the numbers: a field half typed
 * is a field somebody is still typing in, and turning it into a number on
 * every keystroke is how a year being corrected becomes a search nobody
 * asked for.
 */
export interface SearchCriteria {
  name?: string;
  year?: string;
  imdbId?: string;
  /** The identifier at the site the server asks, which names the work
      outright. */
  providerId?: string;
}

/** A kind of picture a work wears. The four the interface draws, and the
 *  four the server prepares. */
export type PictureKind = "poster" | "backdrop" | "thumb" | "logo";

/** One picture the work wears now. */
export interface HeldPicture {
  kind: PictureKind;
  url: string;
  width: number | null;
  height: number | null;
  /** Chosen by hand, which means no later run replaces it. */
  by_hand: boolean;
}

/** One picture the provider holds, offered to be chosen. */
export interface OfferedPicture {
  kind: PictureKind;
  /** What the provider calls it, which is what says which one was chosen. */
  path: string;
  /** Where to show it from: the provider's own address, so choosing among
      thirty of them costs this server nothing. */
  url: string;
  width: number | null;
  height: number | null;
  language: string | null;
  vote_average: number;
  vote_count: number;
}

export type IdentificationNote =
  | "no_match"
  | "provider_unreachable"
  | "provider_busy"
  | "provider_unreadable"
  | "cleared_by_hand";

export interface Page {
  cards: Card[];
  next: string | null;
}

export interface Root {
  label: string;
  /** Its whole path on the server's disk. Shown only on the screen where
      roots are managed: the one place telling two of them apart by more than
      a label matters. Sent to an administrator only. */
  path: string | null;
  access: "missing" | "unreadable" | "read_only" | "read_write";
  explanation_code: string;
  /** What renaming this folder needs. */
  id: string;
}

/** What a library does with its files and with each play, as its settings
 *  and its declaration choose it. */
export type LibraryChoices = Pick<
  Library,
  | "extract_subtitles"
  | "make_thumbnails"
  | "detect_openings"
  | "generate_subtitles"
  | "process_on_arrival"
  | "watch_in_real_time"
  | "keeps_resume_points"
  | "keeps_watched_marks"
>;

export interface Library {
  id: string;
  name: string;
  kind: LibraryKind;
  works: number;
  version: number;
  /** Which heavy readings it wants: the subtitles made of words, the
      pictures of the playback bar, the opening and closing titles. */
  extract_subtitles: boolean;
  make_thumbnails: boolean;
  detect_openings: boolean;
  /** Whether the videos with no subtitle are listened to for one. Only
      offered to a library of personal videos. */
  generate_subtitles: boolean;
  /** Whether those are done as soon as a file arrives, rather than by their
      scheduled tasks. */
  process_on_arrival: boolean;
  /** Whether its folders are watched, and it is scanned again as soon as
      something in them changes. */
  watch_in_real_time: boolean;
  /** Whether it keeps where each account stopped, so a work is picked up
      from there. */
  keeps_resume_points: boolean;
  /** Whether it keeps which works each account has watched. */
  keeps_watched_marks: boolean;
  /** Where that watching stands, for a library that asked for it. */
  watch_state: "watching" | "starting" | "refused" | null;
  /** Why it was refused, as a word turned into a sentence here. */
  watch_refusal: "too_many_folders" | "folder_missing" | "folder_unreadable" | "unavailable" | null;
  /** The language this library's films are described in, as a two letter code.
      Changing it asks the provider about every film again. */
  metadata_language: string;
  roots: Root[];
  /** How many of its files were taken out of it and left on the disk. */
  set_aside: number;
}

/**
 * What taking a library or one of its folders away would take with it.
 *
 * Films the server would stop knowing about, and the files behind them as
 * rows. Not one file leaves the disk: the collection is not this server's to
 * remove.
 */
export interface WouldGo {
  works: number;
  files: number;
}

/** What a deletion did. */
export interface Deleted {
  works: number;
  files: number;
  /** Files looked for on the disk once deleted and not found, when the disk
      was asked to lose them. */
  off_the_disk: number | null;
}

/** What deleting a work would take with it. */
export interface Deletion {
  /** The works and everything under them, each once. */
  works: number;
  /** Every file of it on the disk, copies first, by its whole path. */
  files: { path: string; role: "copy" | "subtitle" | "extra" }[];
  /** Whether this account may delete off the disk as well. */
  may_delete_from_disk: boolean;
  /** Whether the server may write where those files are, tried just now. */
  disks_take_writes: boolean;
}

/** A file taken out of a library while it stays on the disk. */
export interface SetAsideFile {
  /** The folder of the library it sits under, and its path under it: what
      names it when it is taken back. */
  root: string;
  relative_path: string;
  /** Its whole path, for a person to read. */
  path: string;
}

/** One folder of the server's disk, as the picker shows it. */
export interface Folder {
  name: string;
  /** Its whole path: what asking for its contents needs, and what becomes a
      root if it is chosen. */
  path: string;
  /** How many videos sit directly inside, counted up to a bound. No file is
      ever named, here or on the server. */
  videos: number;
  more_videos: boolean;
}

export interface Listing {
  path: string;
  /** Where going up leads, absent at the top of the tree. */
  parent: string | null;
  folders: Folder[];
  cut_short: boolean;
}

/** How much of a library a scan or an identification goes over. */
export type RefreshMode = "new_and_updated_files" | "what_is_missing" | "everything";

/** The three, in the order a menu offers them: lightest first. */
export const REFRESH_MODES: RefreshMode[] = [
  "new_and_updated_files",
  "what_is_missing",
  "everything",
];

/** One of the seven scheduled tasks, each covering every library. */
export type TaskName =
  | "scan"
  | "identify"
  | "ratings"
  | "key_frames"
  | "subtitles"
  | "thumbnails"
  | "openings"
  | "speech"
  | "translation";

/** Where one scheduled task stands. */
export interface ScheduledTask {
  task: TaskName;
  /** Whether it runs by itself every day, and when, in minutes since
      midnight **in UTC**: the server keeps the one clock it can read with
      certainty, and this side turns it into the time of whoever looks. */
  runs_on_schedule: boolean;
  at_utc_minutes: number;
  /** When it next runs by itself, as an instant. Absent when it never does. */
  next_run: string | null;
  /** What it has waiting over every library. Absent for the scan, which
      cannot know what a disk holds until it has walked it. */
  waiting: number | null;
  /** Whether that number counts seasons rather than files. */
  counts_seasons: boolean;
  under_way: boolean;
  /** When it last ran, how that ended, and how long it took. A task that
      never ran and one that ran last night and found nothing look alike
      without them. */
  last_run: string | null;
  last_run_state: string | null;
  last_run_seconds: number | null;
}

export interface ScheduledTasks {
  tasks: ScheduledTask[];
  /** The soonest any task runs by itself. */
  next_run: string | null;
}

/**
 * What the server does with a library, on its own and in what shape.
 *
 * Every one of these was a line of the configuration file, which meant a
 * terminal and a restart to change one.
 */
export interface LibraryWork {
  /** Read the description files some collections keep next to a film. */
  read_companion_files: boolean;
  thumbnails_every_seconds: number;
  thumbnails_height: number;
  thumbnails_columns: number;
  thumbnails_rows: number;
}

/**
 * How the whole server plays films, for every viewer rather than any one of
 * them: what it does about wide gamut colour it cannot show a client, and what
 * it allows the films it converts.
 */
/** One collection as the list of them shows it: gathered by hand, or a saga
 *  the provider knows. */
export interface CollectionSummary {
  id: string;
  name: string;
  made_by_hand: boolean;
  /** How many of its titles this account can reach. */
  count: number;
  /** Its first title, whose poster it wears. */
  cover: Card | null;
}

/** One playlist of this account as the list of them shows it. */
export interface PlaylistSummary {
  id: string;
  name: string;
  count: number;
  cover: Card | null;
}

/** One playlist of this account with its titles, in its order. */
export interface Playlist {
  id: string;
  name: string;
  cards: Card[];
}

/** One collection with its titles, in its order. */
export interface Collection {
  id: string;
  name: string;
  made_by_hand: boolean;
  cards: Card[];
}

/** One subtitle track of a copy, and where it comes from. */
export interface SubtitleTrackInfo {
  id: string;
  language: string | null;
  title: string | null;
  origin: "inside" | "beside" | "downloaded";
  hearing_impaired: boolean;
}

/** One subtitle OpenSubtitles offers for a copy. */
export interface SubtitleOffer {
  file_id: number;
  /** Two letters, sometimes with a region. */
  language: string;
  /** The release it was timed on. */
  release: string;
  downloads: number;
  hearing_impaired: boolean;
  /** Written by a program rather than by somebody. */
  machine_translated: boolean;
  /** Uploaded by somebody OpenSubtitles trusts. */
  trusted: boolean;
  /** Timed on this very file, recognised by its fingerprint. */
  matches_the_file: boolean;
  fps: number | null;
  /** Timed on a video of another speed than this copy, so it drifts. */
  other_speed: boolean;
}

/** Whether a key for OpenSubtitles was given, and an account with it. */
/** A model that listens to personal videos, among the few the server offers. */
export interface SpeechModel {
  id: string;
  bytes: number;
  downloaded: boolean;
  downloading: boolean;
}

/** What listens to personal videos: whether the tool is on the machine, the
    model in use, and every model offered. */
export type SpeechEffort = "quiet" | "balanced" | "maximum";

/** The model that translates what listening wrote into French. */
export interface TranslationModel {
  bytes: number;
  downloaded: boolean;
  downloading: boolean;
}

export interface SpeechStatus {
  tool_found: boolean;
  translation: TranslationModel;
  chosen: string | null;
  /** How much of the processor listening takes. */
  effort: SpeechEffort;
  models: SpeechModel[];
}

export interface OpenSubtitlesSettings {
  has_key: boolean;
  signed_in: boolean;
  tried: "kept" | "refused" | "unreachable" | null;
}

/** A rating from elsewhere than the provider: out of ten for IMDb, with how
 *  many voted, and out of a hundred for Rotten Tomatoes' critics. */
export interface Rating {
  source: "imdb" | "rotten_tomatoes";
  value: number;
  votes: number | null;
}

/** Where the ratings from elsewhere stand. The OMDb key itself never comes
 *  back, only whether there is one. */
export interface RatingsSettings {
  /** When IMDb's file of ratings was last fetched, as an instant. */
  imdb_fetched_at: string | null;
  has_omdb_key: boolean;
  /** What trying the key just typed came to. */
  tried: "kept" | "refused" | "unreachable" | null;
}

/** One graphics card that passed its trials, as it is offered to convert
    films. */
export interface CardOffered {
  /** What it is chosen by. */
  key: string;
  name: string;
  /** The path it is driven by: vaapi, cuda. */
  way: string;
  /** The codecs it was proved to write, and to read. */
  writes: VideoCodec[];
  reads: string[];
  converts_wide_gamut: boolean;
  paints_picture_subtitles: boolean;
}

/** The cards there are to choose from, and which one converts films. */
export interface CardChoice {
  cards: CardOffered[];
  /** The key that was chosen, or null when the server picks. */
  chosen: string | null;
  /** The key of the card converting films now. */
  in_use: string | null;
  /** Whether the card that was chosen did not pass its trials this time. */
  chosen_missing: boolean;
  /** Whether a film that card cannot take goes to another card that can. */
  other_card_when_refused: boolean;
  /** Whether a film the card refuses while playing goes to the processor,
      rather than stopping. */
  processor_when_refused: boolean;
}

export interface PlaybackSettings {
  /** Never convert such colour, even where a client cannot show it correctly.
      Off by default. Dolby Vision without a compatible base layer is
      converted regardless, since left alone it looks broken rather than
      merely washed out. */
  tone_mapping_disabled: boolean;
  /** How many films the server may convert at once; null for no ceiling,
      which is where every server starts. */
  max_transcoding_sessions: number | null;
  /** How much of the disk the segments of those films may fill, in
      megabytes; null for no ceiling. */
  transcode_cache_megabytes: number | null;
  /** How far behind each viewer those segments stay whatever the ceiling
      says, in seconds. */
  transcode_kept_behind_seconds: number;
  /** The codecs a converted film may come out in, best first. Never empty. */
  transcode_video_codecs: VideoCodec[];
}

/** A codec a converted film may come out in. */
export type VideoCodec = "av1" | "hevc" | "h264";

export interface Filters {
  genres: { name: string; works: number }[];
  decades: { decade: number; works: number }[];
  /** The letters titles really start with, the bucket for the rest first. */
  initials: { name: string; works: number }[];
  /** How many works still wait for a name. */
  awaiting_identification: number;
}

/** Why a work opens the page, which is what its button says. */
export type Because = "started" | "pinned" | "new" | "suggested";

/** One work the page opens on, shown large rather than as a card. */
export type HeroItem = Card & {
  because: Because;
  /** The wide picture behind it, and its title as its own designers drew it.
      Both empty for a work nobody has looked up, which the banner answers by
      writing the title out instead. */
  backdrop: Picture[];
  logo: Picture[];
  tagline: string | null;
  overview: string | null;
  genres: string[];
  /** What the file itself holds, for the badges beside the title. Raw as the
      file states it: turning it into "4K" or "Dolby Atmos" is drawing. */
  width: number | null;
  height: number | null;
  hdr: string | null;
  sound: string | null;
  /** Set where the work shown large is an episode: the picture and the drawn
      title above already come from the series, and these place it in words. */
  series_title: string | null;
  season_number: number | null;
  episode_number: number | null;
};

export interface Home {
  /** The few works the page opens on, largest of all. */
  hero: HeroItem[];
  /** Whether they were drawn at random. */
  hero_at_random: boolean;
  /** Films and episodes started and not finished, the latest first. An
      episode carries the series it hangs under, which is what the card leads
      with: nobody left off in the middle of an episode title. */
  carry_on: (Card & {
    position_seconds: number;
    series: string | null;
    series_title: string | null;
    season_number: number | null;
    episode_number: number | null;
  })[];
  /** The episode each started series is waiting on, with the series it
      belongs to: an episode's own title is not what anybody remembers. */
  up_next: (Card & {
    series: string;
    series_title: string;
    season_number: number | null;
    episode_number: number | null;
  })[];
  recently_added: Card[];
  /** One row per kind of library this server really holds: its newest
      works for the row of that kind, and the newest of them somebody named
      for the fan of posters on the tile that leads to it. */
  shelves: { kind: LibraryKind; cards: Card[]; fan: Card[] }[];
  works: number;
  awaiting_identification: number;
  /** The sections this viewer shows below the banner, in their order. */
  sections: HomeSection[];
  /** The kinds of library in the order this viewer chose, which the tiles
      leading to them follow, music's among them. */
  kind_order: LibraryKind[];
}

/** One section of the home page below its banner: the newest of each kind
 *  of library is a section of its own. */
export type HomeSection =
  | "band"
  | "carry_on"
  | "up_next"
  | "recently_added"
  | `newest:${LibraryKind}`;

/** One of the buttons of the bar at the top, on it or in the account's menu
 *  next to it. */
export type HeaderButton =
  | "search"
  | "favourites"
  | "watch_later"
  | "collections"
  | "playlists"
  | "requests"
  | "notifications"
  | "scan"
  | "administration"
  | "cast"
  | "settings";

export interface Credit {
  /** Where their own page is. */
  person_id: string;
  name: string;
  role: string;
  character: string | null;
  /** Empty for anyone whose face has not been fetched. */
  photo: Picture[];
}

export interface VideoTrack {
  /** The title the file itself carries, when it carries one. */
  title: string | null;
  codec: string;
  profile: string | null;
  level: number | null;
  width: number;
  height: number;
  aspect_ratio: string | null;
  is_interlaced: boolean;
  hdr: string | null;
  frame_rate: number | null;
  bitrate: number | null;
  pixel_format: string | null;
  reference_frames: number | null;
  color_primaries: string | null;
  color_space: string | null;
  color_transfer: string | null;
  bit_depth: number | null;
  is_default: boolean;
}

export interface AudioTrack {
  title: string | null;
  codec: string;
  profile: string | null;
  language: string | null;
  channels: number;
  channel_layout: string | null;
  sample_rate: number | null;
  bit_depth: number | null;
  bitrate: number | null;
  is_default: boolean;
  is_forced: boolean;
}

export interface SubtitleTrack {
  title: string | null;
  codec: string;
  language: string | null;
  is_default: boolean;
  is_forced: boolean;
  is_hearing_impaired: boolean;
  is_external: boolean;
  burns_in: boolean;
}

export interface Version {
  id: string;
  summary: string;
  /** Where the file is, root included, and the disk it is on. Sent to an
      administrator only. */
  path: string | null;
  root_label: string | null;
  added_at: string;
  size_bytes: number;
  duration_minutes: number | null;
  overall_bitrate: number | null;
  container: string | null;
  analysed: boolean;
  missing: boolean;
  video: VideoTrack[];
  audio: AudioTrack[];
  subtitles: SubtitleTrack[];
  chapters: number;
}

export interface Trailer {
  name: string | null;
  remote_url: string | null;
  local: boolean;
  /** Where to fetch a local one. Absent for a link, which is watched where it
   *  lives: this server does not go and fetch someone else's video. */
  url: string | null;
}

/** One work hanging under another: a season of a series, an episode of a
 *  season, what a folder of one's own holds. */
/** One work hanging under another: its card, and what its place under the
 *  other adds to it. */
export interface Child {
  /** The season number, the episode number. */
  number: number | null;
  /** The name it carries, when that name says anything its number does not.
   *  A page shows the number in the language it is being read in. */
  title: string | null;
  /** What it is about, for a list of episodes that has room to say it. */
  overview: string | null;
  /** How many hang under it: a season's episodes, a folder's contents. */
  child_count: number;
  /** It as every row draws it, with what this viewer made of it. */
  card: Card;
}

/** The episode a page offers to play next. */
export interface NextEpisode {
  id: string;
  season: number | null;
  episode: number | null;
  /** Its own name, when it has one its number does not already say. */
  title: string | null;
  /** The file it plays from, so a button starts it without asking again. */
  source_id: string | null;
}

/** One work this one hangs under, as a way back to it. */
export interface Ancestor {
  id: string;
  kind: string;
  number: number | null;
  title: string;
  /** The series' own mark, drawn as it draws its title. Empty for anything
   *  that is not a series. */
  logo: Picture[];
}

export interface Work {
  id: string;
  library_id: string;
  kind: string;
  title: string;
  tagline: string | null;
  overview: string | null;
  year: number | null;
  /** The day it came out, and for a series the day it ended, year, month,
   *  day. */
  release_date: string | null;
  end_date: string | null;
  runtime_minutes: number | null;
  rating: number | null;
  age_rating: string | null;
  identification: Card["identification"];
  identification_note: IdentificationNote | null;
  color: string | null;
  /** Which season or episode this is, for a page that draws its own number. */
  number: number | null;
  /** Whether the title above says anything its number does not. False for a
   *  season a scan could only number, whose name is drawn from the number in
   *  the language the page is being read in. */
  has_own_name: boolean;
  genres: string[];
  studios: string[];
  collection: string | null;
  cast: Credit[];
  crew: Credit[];
  poster: Picture[];
  backdrop: Picture[];
  /** The title drawn as the film draws it, shown in place of the title
   *  written out. Empty for a film the provider draws under none. */
  logo: Picture[];
  versions: Version[];
  trailers: Trailer[];
  external_ids: { provider: string; id: string }[];
  /** What IMDb's viewers and Rotten Tomatoes' critics made of it, when the
   *  server has heard. */
  ratings: Rating[];
  /** The seasons of a series, the episodes of a season, in order. Empty for
   *  anything met on its own. */
  children: Child[];
  /** Every episode of an episode's season, itself included, in order. Empty
   *  for anything that is not an episode. */
  siblings: Child[];
  /** The way back up, nearest first. Empty for anything met on its own. */
  ancestry: Ancestor[];
  /** The episode to watch next: the first one left on a series or a season,
   *  the one after this on an episode. Absent when there is none. */
  carry_on_with: NextEpisode | null;
  /** The episode before this one, so the player can offer to step back into
   *  it. Absent for anything that is not an episode, and for the first
   *  episode of a series. */
  previous_episode: NextEpisode | null;
  /** The photos before and after this one in its folder. Absent for anything
   *  that is not a photo, and at either end of the folder. */
  previous_photo: string | null;
  next_photo: string | null;
  /** This work as its card, with what this viewer made of it: the page marks
   *  it through the same card every row draws. */
  card: Card | null;
  /** Works like this one by a genre they share, and that genre. Absent for
   *  anything but a film or a series, and when nothing shares a genre. */
  alike: { genre: string; cards: Card[] } | null;
  /** The saga a film belongs to and every film of it here, the first to
   *  come out first, this one among them, then the films where its
   *  characters come back. Without a name for a film in no saga or alone
   *  of its saga here, whose row holds only those films. */
  saga: { name: string | null; cards: Card[] } | null;
}

/** One person, and what of theirs this server holds. */
export interface Person {
  id: string;
  name: string;
  biography: string | null;
  /** Year, month and day, as the provider writes them. */
  born_on: string | null;
  died_on: string | null;
  birthplace: string | null;
  photo: Picture[];
  /** What of theirs this account can open, newest first. */
  works: Card[];
}

export interface Job {
  id: string;
  kind: string;
  state: "queued" | "running" | "succeeded" | "failed" | "cancelled" | "interrupted";
  target: string | null;
  /** Which pass the job is on. The counters below count that pass alone, so
   *  a bar dropping back to nothing is a pass ending, not a crash. */
  step: string | null;
  /** The file it is on at this very moment, when the pass says so. */
  doing: string | null;
  done: number;
  total: number | null;
  ratio: number | null;
  failure_reason: string | null;
}

export interface Jobs {
  running: Job[];
  recent: Job[];
}

/** One line the server said, with the tag saying which part said it. */
export interface JournalLine {
  at: string;
  /** error, warn, info, debug or trace. */
  level: string;
  /** One word for the part of the server that wrote it. */
  tag: string;
  /** The module it came from, for finding the line in the source. */
  module: string;
  message: string;
}

/** What the server has been saying, plus the tags it really wrote. */
export interface Journal {
  tags: { name: string; lines: number }[];
  lines: JournalLine[];
}

/** What a screen asks the journal for. */
export interface JournalQuery {
  tags?: string[];
  holding?: string;
  most?: number;
}

/**
 * What moved the film, as far as the page can tell.
 *
 * A click and a drag are not the same gesture at all and look identical once
 * the film has moved: a drag holds the library back for as long as the hand is
 * down and sets it going once, a click holds it back and sets it going again in
 * the same breath, and clicking along the bar does that once per click. What
 * the page never asked for is the last of them, and it is the one worth
 * catching: it is the browser or the library moving the film by itself.
 */
export type HowItMoved =
  | "a_click"
  | "a_drag"
  | "a_step"
  | "picked_up_where_it_was_left"
  | "not_the_page";

/** How a film failed to flow for a moment as it started: a picture held on
 *  the screen too long, pictures of the film never shown, or the page too
 *  busy to look while the browser went on showing them. */
export type HitchKind = "held" | "skipped" | "blind";

/** One moment the film did not flow as it should have. */
export interface OpeningHitch {
  kind: HitchKind;
  /** How long after the first picture, in milliseconds. */
  at_ms: number;
  /** The moment of the film, in seconds. */
  at_second: number;
  /** How long between the two pictures, in milliseconds. */
  gap_ms: number;
  /** Pictures of the film that were never shown, for a skip. */
  pictures_lost: number;
  /** How much of the gap the page spent busy with its own work. */
  busy_ms: number;
  /** How far the film's own clock went during the gap: about the gap itself
   *  when the clock ran on under a picture that stood still, next to nothing
   *  when the clock itself stopped. Absent when it could not be read. */
  clock_ms: number | null;
  /** How long the browser says the picture after the gap took to decode,
   *  when it says. */
  decoded_ms: number | null;
  /** Whether the picture after the gap came out a different size, which is
   *  the decoder having been set up again. */
  resized: boolean;
}

/** Something that changed around the film while it opened. */
export type PageChangeKind =
  | "uncovered"
  | "controls_away"
  | "controls_back"
  | "waiting"
  | "stalled"
  | "playing"
  | "picture_resized"
  | "box_resized"
  | "fullscreen"
  | "tab_hidden"
  | "tab_shown"
  | "piece_added";

/** One such change, and when, from the first picture. */
export interface PageChange {
  what: PageChangeKind;
  at_ms: number;
}

/** What the opening seconds of a film came to, picture by picture. */
export interface OpeningSeconds {
  /** How long was followed, from the first picture, in milliseconds. */
  over_ms: number;
  pictures: number;
  /** How long one picture of this film lasts, as measured. */
  picture_ms: number;
  held: number;
  worst_held_ms: number;
  skipped: number;
  pictures_lost: number;
  blind: number;
  worst_blind_ms: number;
  /** The first few, in order. */
  first_hitches: OpeningHitch[];
  /** Pictures the browser itself counts as decoded and thrown away. */
  pictures_dropped: number;
  /** How many times the browser said it was waiting for more of the film. */
  waited: number;
  /** Spells of fifty milliseconds or more the page spent on its own work,
   *  and how long they came to. Absent when the browser cannot say. */
  busy_spells: number | null;
  busy_ms: number | null;
  /** Films this tab started before this one: the first is the cold one. */
  films_before_in_this_tab: number;
  /** How long the page had been open when the first picture came up. */
  page_age_ms: number;
  /** Whether the film started with its sound on. The browser's clock runs
   *  on the sound, so a sound card slow to wake holds the pictures too. */
  sound_on: boolean;
  /** What changed around the film while it opened, in order: a picture that
   *  stops the moment something drawn over it goes away points at how the
   *  browser puts video on the screen rather than at the film. */
  page_changes: PageChange[];
}

/**
 * A fact the page may tell the journal.
 *
 * The server defines the list and refuses anything outside it, so this type is
 * that list and never a message of the page's own making. Half of what happens
 * when a film starts happens here rather than on the server, and the journal
 * showed none of it.
 */
export type PageFact =
  | {
      saw: "playback_began";
      /** What the playlist told the library, when it read it. */
      playlist_said_second: number | null;
      /** Where it settled on beginning. */
      began_at_second: number;
      /** The first segment it asked the server for. */
      first_segment: number;
    }
  | {
      saw: "viewer_jumped";
      from_second: number;
      to_second: number;
      was_playing: boolean;
      moved_by: HowItMoved;
    }
  | {
      saw: "the_picture_arrived";
      /** How wide and tall the browser says the picture is meant to be shown. */
      across: number;
      down: number;
      /** The box the page is drawing it in. */
      drawn_across: number;
      drawn_down: number;
    }
  | {
      saw: "the_picture_came_back";
      /** Where the viewer had asked to land. */
      asked_for_second: number;
      /** The moment of the film on the picture that came up. */
      showed_second: number;
      /** How long after the jump. */
      after_ms: number;
      /** Whether the browser already held that moment when the jump was made. */
      was_held_already: boolean;
      /** How many separate stretches it held when the jump was made. */
      stretches: number;
    }
  | {
      saw: "playback_stalled";
      at_second: number;
      /** Whether the browser was still trying to move to a new place. */
      was_seeking: boolean;
      /** The stretch the browser holds around that moment, when it holds one. */
      held_from_second: number | null;
      held_to_second: number | null;
      /** How many separate stretches it holds. More than one means a hole. */
      stretches: number;
      ready_state: number;
      pictures_shown: number;
    }
  | {
      saw: "playback_picked_up_again";
      at_second: number;
      waited_ms: number;
    }
  | {
      saw: "the_picture_stood_still";
      at_second: number;
      for_ms: number;
      pictures_shown: number;
      /** Pictures decoded and thrown away rather than shown. */
      pictures_dropped: number;
      /** What the browser held around that moment, when it held anything. */
      held_from_second: number | null;
      held_to_second: number | null;
      stretches: number;
      ready_state: number;
      /** Whether the page went out of sight while this lasted. */
      page_was_hidden: boolean;
    }
  | {
      saw: "pictures_were_dropped";
      at_second: number;
      /** The stretch this counts over, in milliseconds. */
      over_ms: number;
      pictures_shown: number;
      pictures_dropped: number;
      /** Seconds of film the browser holds in all, behind and ahead. */
      held_seconds: number;
      /** What the page weighs in memory, where the browser says. */
      page_memory_mb: number | null;
    }
  | {
      saw: "picture_and_sound";
      at_second: number;
      over_ms: number;
      samples: number;
      /** Positive when the picture is behind the sound. */
      picture_behind_ms: number;
      worst_behind_ms: number;
    }
  | {
      saw: "segment_placed";
      segment: number;
      playlist_second: number;
      video_starts_second: number | null;
      audio_starts_second: number | null;
      library_shift_second: number | null;
    }
  | ({ saw: "the_opening_seconds" } & OpeningSeconds)
  | {
      saw: "playback_refused";
      /** Why the library gave up, in its own words. Cut short by the server. */
      because: string;
      /** Whether the film went on playing, read by the browser itself. */
      browser_took_over: boolean;
    }
  | {
      saw: "library_hiccup";
      /** What the library met and got over by itself, in its own words. */
      because: string;
      segment: number | null;
    }
  | {
      saw: "how_it_started";
      /** How long after the browser was handed the film. */
      after_ms: number;
      at_second: number;
      /** Whether the browser left it paused, which nobody asked for this early. */
      paused: boolean;
      ready_state: number;
      network_state: number;
      pictures_shown: number;
      pictures_dropped: number;
      held_from_second: number | null;
      held_to_second: number | null;
      stretches: number;
      /** The browser's own code for what went wrong, when it says anything did. */
      error_code: number | null;
    }
  | {
      saw: "how_it_decodes";
      /** The graphics card as the browser names it, when it names one. */
      card: string | null;
      codec: string;
      across: number;
      down: number;
      frames_per_second: number | null;
      /** Whether the browser says it decodes this on the card. */
      on_the_card: boolean | null;
      /** Whether it says it keeps up. */
      smoothly: boolean | null;
    }
  | {
      saw: "loading_stage";
      /** One of the real moments on the way to a film playing. */
      stage:
        | "opening"
        | "session_opened"
        | "manifest_parsed"
        | "producing"
        | "produced"
        | "first_fragment_loaded"
        | "done";
      /** How long the stage before this one took, in milliseconds. */
      after_ms: number;
    }
  | {
      saw: "subtitles_read";
      cues: number;
      first_start_second: number | null;
      last_end_second: number | null;
      /** Cues that begin before the one ahead of them. */
      out_of_order: number;
      /** Cues that end before they begin or the moment they do. */
      empty_or_backwards: number;
      hidden: boolean;
    }
  | {
      saw: "subtitles_on_screen";
      why: "changed" | "jumped";
      at_second: number;
      /** How many cues the browser holds in all. */
      cues: number;
      /** What the browser shows, and what the times of the file give for this moment. */
      shown: SubtitleCue[];
      expected: SubtitleCue[];
    };

/** One subtitle as the journal is told of it: its times and the start of its words. */
export interface SubtitleCue {
  start_second: number;
  end_second: number;
  text: string;
}

/** The reading a fact is about: the session a rebuilt film is fed from, or the
 *  source of one handed over as it is, for which no session is ever opened. */
export type Reading = { session: string } | { source: string };

export type PageSaw = Reading & PageFact;

/** What the summary of the administration says about the server. */
export interface Overview {
  server_name: string;
  version: string;
  started_at: string;
  /** Whether the database runs the way it has to. */
  database_ready: boolean;
  media_tools: {
    found: boolean;
    version: string | null;
    /** What the graphics card converting films is called, when one was
        proven to work. */
    card: string | null;
    /** Whether that card still opens. */
    card_opens: boolean;
  };
  accounts: number;
  devices: number;
  /** Devices used in the last day. */
  devices_today: number;
  /** Everything that failed its check, each by name. Empty means all is well. */
  worries: Worry[];
}

/** One thing that failed its check on the server. */
export type Worry =
  | { kind: "database_mode" }
  | { kind: "database_refuses_writes" }
  | { kind: "media_tools_missing" }
  | { kind: "card_unreachable" }
  | { kind: "folder_missing"; label: string }
  | { kind: "disk_nearly_full"; mount: string; used: number };

/** What the machine spent over one stretch of time. */
export interface MeasurePoint {
  at: string;
  /** Shares from nought to one. */
  processor: number | null;
  memory_used: number;
  memory_total: number;
  load: number | null;
  /** Bytes a second. */
  received: number;
  sent: number;
  /** Nothing when the server has no card to measure. */
  card: number | null;
  /** Degrees, when the machine lets them be read. */
  temperature: number | null;
}

/** The live figures, and the disks under every folder the server uses. */
export interface LiveMeasures {
  machine: { processor: string | null; threads: number };
  /** The last ten minutes, a point every two seconds. */
  recent: MeasurePoint[];
  disks: MeasuredDisk[];
}

/** One disk, by where it is mounted and by what it holds. */
export interface MeasuredDisk {
  mount: string;
  /** The libraries with a folder on it, by name. */
  libraries: string[];
  /** Whether the server keeps its own data, cache or conversions on it. */
  holds_the_server: boolean;
  total_bytes: number;
  available_bytes: number;
}

/** How far back the curves reach. */
export type MeasuredOver = "hour" | "day" | "week" | "month" | "year";

export interface SystemInfo {
  server_name: string;
  version: string;
  api_version: string;
  setup_complete: boolean;
  playback_available: boolean;
  maintenance: boolean;
}

/**
 * A failure carrying the code the server sent.
 *
 * The code is what the interface words in its own language; the server never
 * sends a finished sentence, precisely so that it can be translated here.
 */
export class ApiError extends Error {
  constructor(
    readonly code: string,
    readonly status: number,
    /** The word saying which thing to put right, when the server sent one.
        A refusal somebody meets while filling a form in has to say which
        field, and `invalid_input` alone says nothing anybody can act on. */
    readonly reason?: string,
    /** Everything else the refusal carried, such as how long to wait before
        asking again. Values, never a sentence: the wording is this side's. */
    readonly details?: Record<string, unknown>,
  ) {
    super(code);
  }
}

/**
 * The addresses where a refusal is a wrong password rather than a session that
 * has ended.
 *
 * Everywhere else, being told that nobody is signed in means the session is
 * over and the interface has to say so and show the door again. At these
 * three it means what was typed is wrong, and throwing somebody back to a
 * fresh door would lose what they typed and tell them nothing.
 */
const THE_DOOR = ["/api/v1/session", "/api/v1/setup", "/api/v1/me/password"];

/** Told when the server says nobody is signed in any more. */
let noticeOfTheDoorClosing: (() => void) | null = null;

/**
 * Asks to be told when a session ends, wherever it ends.
 *
 * A session can end between two clicks: it was signed out on another machine,
 * the server was restarted and swept it, or it simply ran out. Without this,
 * an interface carries on drawing a library it can no longer read and every
 * click fails in silence.
 */
export function whenTheDoorCloses(listener: () => void) {
  noticeOfTheDoorClosing = listener;
}

/**
 * The one exchange with the server. Everything below is what is asked for.
 *
 * Written once because it was written three times: a plain read, a read of
 * text the server rendered, and a change sent to it. The three had already
 * come apart, and the one that reads text had stopped carrying back the word
 * the server sends to say what it refused, so a refusal on that road arrived
 * as "something went wrong" and nothing else.
 *
 * Two things happen here and nowhere else. A question this interface walked
 * away from is handed on as it is, because that is not the server failing to
 * answer: shown to a viewer it reads as a fault, on a screen they have
 * already left. And a refusal is unwrapped into the code the server sent and
 * the field it named, which is what lets a form say which box is wrong.
 */
async function exchange(
  path: string,
  accept: string,
  method?: string,
  body?: unknown,
  signal?: AbortSignal,
): Promise<Response> {
  let response: Response;
  try {
    response = await fetch(path, {
      method,
      /* A file is sent as it is, under its own type: an image sent as the
         characters of a JSON string would be three times its size and
         nothing the server could read. */
      headers:
        body === undefined
          ? { accept }
          : { accept, "content-type": body instanceof Blob ? body.type : "application/json" },
      body: body === undefined ? undefined : body instanceof Blob ? body : JSON.stringify(body),
      signal,
    });
  } catch (cause) {
    if (cause instanceof DOMException && cause.name === "AbortError") {
      throw cause;
    }
    throw new ApiError("unreachable", 0);
  }

  if (!response.ok) {
    const said = await response.json().catch(() => null);
    if (response.status === 401 && !THE_DOOR.includes(path.split("?")[0])) {
      noticeOfTheDoorClosing?.();
    }
    throw new ApiError(
      said?.code ?? "generic",
      response.status,
      said?.details?.reason,
      said?.details,
    );
  }
  return response;
}

/** Reads one answer of the server. Exported for the parts of the interface
 *  kept apart from this file, music among them, which ask the same way. */
export async function get<T>(path: string, signal?: AbortSignal): Promise<T> {
  return (await exchange(path, "application/json", undefined, undefined, signal)).json() as Promise<T>;
}

/** Reads one answer of the server as it came, for what is neither JSON nor
 *  text. */
export function getRaw(path: string, accept: string, signal?: AbortSignal): Promise<Response> {
  return exchange(path, accept, undefined, undefined, signal);
}

/** Turns what a screen ticked into the query the server reads. */
function journalQuery(query: JournalQuery): string {
  const parts = new URLSearchParams();
  if (query.tags && query.tags.length > 0) parts.set("tags", query.tags.join(","));
  if (query.holding) parts.set("holding", query.holding);
  if (query.most) parts.set("most", String(query.most));
  return parts.toString();
}

/** The same as above for a report the server renders itself. */
async function getText(path: string, signal?: AbortSignal): Promise<string> {
  return (await exchange(path, "text/plain", undefined, undefined, signal)).text();
}

export const post = <T>(path: string, body?: unknown, signal?: AbortSignal) =>
  send<T>("POST", path, body, signal);
export const remove = <T>(path: string, signal?: AbortSignal) =>
  send<T>("DELETE", path, undefined, signal);
export const put = <T>(path: string, body?: unknown, signal?: AbortSignal) =>
  send<T>("PUT", path, body, signal);

async function send<T>(
  method: string,
  path: string,
  body?: unknown,
  signal?: AbortSignal,
): Promise<T> {
  return (await exchange(path, "application/json", method, body, signal)).json() as Promise<T>;
}

/** Handed to the browser to deliver while the page goes away: a request
 *  started as a tab closes is usually dropped, and this one is not. */
function handOver(path: string, body: unknown): void {
  navigator.sendBeacon?.(path, new Blob([JSON.stringify(body)], { type: "application/json" }));
}

export interface PlaybackTrack {
  id: string;
  language: string | null;
  title: string | null;
  codec: string;
  is_default: boolean;
  /** Audio only. */
  channels: number | null;
  /** Subtitles only: showing it means rebuilding the picture. */
  burns_in: boolean | null;
  /** Where to fetch the words. Absent for a soundtrack, and for a subtitle
   *  made of pictures: there is no text in one to hand over. */
  url: string | null;
}

export interface PlaybackPlan {
  url: string;
  /** The tracks this answer was worked out for, chosen or fallen back to. */
  chosen_audio_id: string | null;
  chosen_subtitle_id: string | null;
  /** direct_play, remux, transcode_audio or full_transcode. */
  method: string;
  expensive: boolean;
  /** Codes with their values, which this interface turns into sentences. */
  reasons: { code: string; [key: string]: unknown }[];
  duration_minutes: number | null;
  resume_from_seconds: number | null;
  audio: PlaybackTrack[];
  subtitles: PlaybackTrack[];
  /** What is rebuilding the picture, when something is. */
  rebuild: PictureRebuild | null;
  /** The little pictures of the bar, when this film has been read for them. */
  thumbnails: PlaybackThumbnails | null;
  /** Where the film changes scene, when its file names them. Empty for most. */
  chapters: PlaybackChapter[];
  /** The stretches nobody wants to sit through, when the file says where they
   *  are. Empty for most files. */
  segments: PlaybackSegment[];
  /** The kinds of stretch a person corrected by hand in this file. */
  corrected_segments: string[];
  /** What the file itself holds, beside what is being made of it. */
  film: FilmHolds;
  /** What is done with the film's wide gamut colour, when it has any. */
  wide_gamut: WideGamutHandling | null;
}

/** What is done with a film's wide gamut colour. */
export interface WideGamutHandling {
  /** Converted to standard range, rather than handed over as it is. */
  converted: boolean;
  /** Whether the viewer's choice decided it. When it did not, choosing
   *  otherwise changes nothing, and the player does not offer to. */
  follows_choice: boolean;
}

/** What a viewer wants done with a film of wide gamut colour. */
export type WideGamutChoice = "automatic" | "always_convert" | "never_convert";

/** What is drawn behind the pages: a painting of light, the drawn shelf, or
 *  nothing. */
export type Backdrop = "light" | "library" | "none";

/** When a work left partway counts as started, as watched, or as too short
 *  to come back to: below the smallest share it starts again, from the
 *  largest it is watched, shorter than the length it is never carried on. */
export interface ResumeRules {
  min_percent: number;
  max_percent: number;
  min_seconds: number;
}

/** When a film starts with subtitles nobody picked for it. */
export type SubtitleMode = "always" | "smart" | "only_forced" | "from_the_file" | "never";

/** One stretch of a film a button offers to skip. */
export interface PlaybackSegment {
  /** recap, intro, outro or advertisement. */
  kind: string;
  from_second: number;
  to_second: number;
  /** Who said where it is: the file's chapters, the listening, or a person. */
  origin: "chapter" | "detected" | "manual";
}

/** What a player offers to skip in a file, and the kinds a person spoke for
 *  there, which includes a kind said to have none. */
export interface Stretches {
  segments: PlaybackSegment[];
  corrected_segments: string[];
}

/** What a person says about one kind of stretch in one file. */
export type SegmentCorrection =
  | { said: "stretch"; from_second: number; to_second: number }
  | { said: "none" };

/**
 * One place the film changes scene.
 *
 * Marked on the bar so that a viewer sees the shape of a film rather than one
 * unbroken line, and so that landing on the start of a scene is something the
 * bar helps with.
 */
export interface PlaybackChapter {
  at_second: number;
  /** What the file calls it, when it calls it anything. */
  title: string | null;
}

/** What the file holds, as the analyser read it. */
export interface FilmHolds {
  container: string | null;
  size_bytes: number;
  /** Everything in the file per second, streams and container alike. */
  overall_bitrate: number | null;
  picture: PictureHeld | null;
  /** The soundtrack being played, not the first one in the file. */
  sound: SoundHeld | null;
}

export interface PictureHeld {
  codec: string;
  profile: string | null;
  /** The picture, margins off. */
  width: number;
  height: number;
  /** The frame it sits in, only when the film says part of it is not the
   *  picture. */
  frame_width: number | null;
  frame_height: number | null;
  frame_rate: number | null;
  bitrate: number | null;
  hdr: string | null;
  bit_depth: number | null;
}

export interface SoundHeld {
  codec: string;
  channels: number;
  channel_layout: string | null;
  sample_rate: number | null;
  bitrate: number | null;
}

/**
 * The little pictures shown while dragging along the bar.
 *
 * They come as sheets of many, and the page cuts one out with a background
 * offset: one request covers a hundred of them.
 */
export interface PlaybackThumbnails {
  /** Where the sheets are. A slash, the sheet number and `.jpg` go after it. */
  url: string;
  /** How far apart in the film two of them stand. */
  every_seconds: number;
  /** Size of one thumbnail on a sheet, in pixels. */
  width: number;
  height: number;
  /** How many stand across one sheet and how many down it. */
  columns: number;
  rows: number;
  /** How many the film has. Past the last one a sheet holds only black. */
  counted: number;
}

/** What is rebuilding the picture, and into what. */
export interface PictureRebuild {
  /** card or processor. Which card is a fact about the machine, and stays in
   *  the report where the administrator reads it. */
  by: "card" | "processor";
  codec: string;
  height: number | null;
  /** Rate the picture is held to, in bits per second. */
  bitrate: number | null;
}

/** A film being converted as it is watched. */
export interface PlaybackSession {
  id: string;
  /** What the player is pointed at: the whole film, listed before any of it
   *  has been produced, so a viewer can jump anywhere at once. */
  playlist_url: string;
  duration_minutes: number | null;
  resume_from_seconds: number | null;
}

/** What the viewer has decided, and what they can decide between. */
export interface ViewerPreferences {
  /** The language the interface speaks to this person, as two letters. Theirs
      rather than the browser's, so signing in on another machine carries it. */
  interface_language: string;
  /** light, dark or system. */
  theme_mode: string;
  /** The colour everything active is drawn in, as a hash and six digits. */
  accent_color: string;
  /** Three letter code, or null for no preference: the file then decides. */
  preferred_audio_language: string | null;
  preferred_subtitle_language: string | null;
  subtitle_mode: SubtitleMode;
  subtitle_modes: SubtitleMode[];
  downmix_method: string;
  downmix_gain: number;
  /** The range the gain is kept inside, so a slider cannot be dragged
   *  somewhere the server would refuse. */
  downmix_gain_range: [number, number];
  downmix_methods: string[];
  /** How tall the banner of the home page is, as a share of the screen's
   *  width: what it really sets is how much of the picture behind it
   *  survives, since those pictures are sixteen by nine. */
  banner_height: number;
  banner_height_range: [number, number];
  /** Where a band is cut out of that picture, nought at its top and one at
   *  its foot. */
  banner_cut: number;
  /** Whether the banner draws a fresh handful every time the page opens. */
  banner_at_random: boolean;
  /** Whether the home page opens on its banner at all. */
  banner_shown: boolean;
  /** Whether it takes the whole window, the height then deciding nothing. */
  banner_fills_the_screen: boolean;
  /** Whether the bar at the top slides away while a page is read down. */
  header_hides_on_scroll: boolean;
  /** The same on a phone, chosen apart. */
  header_hides_on_scroll_phone: boolean;
  /** Every button of the bar and of the account's menu, in the order both
   *  show them, and those shown on the bar. */
  header_buttons: HeaderButton[];
  buttons_in_the_bar: HeaderButton[];
  /** What is drawn behind the pages, and which painting of light, from one. */
  backdrop: Backdrop;
  backdrop_light: number;
  /** Whether this account is left off the list the sign in screen offers.
   *  Hidden, it still signs in: the name is typed rather than pressed. */
  hidden_at_the_door: boolean;
  /** Every kind of library, in the order the home page lays them out. */
  home_order: LibraryKind[];
  /** Every section of the home page below its banner, in its order, and
   *  those it leaves off. */
  home_sections: HomeSection[];
  hidden_home_sections: HomeSection[];
  /** How far the player's two step buttons jump, in seconds. */
  step_back_seconds: number;
  step_on_seconds: number;
  /** The longest a step, or the way back on resuming, may be, in seconds. */
  longest_step: number;
  /** How far back a film starts from where it was left, nought for none. */
  resume_rewind_seconds: number;
  /** When a work left partway counts as started, as watched, or as too
   *  short to come back to, for every kind of library. */
  resume_rules: ResumeRules;
  /** Whether each kind of library has rules of its own, and those it has. */
  resume_rules_per_kind: boolean;
  resume_rules_by_kind: (ResumeRules & { kind: LibraryKind })[];
  /** The smallest share at most, the largest at least, the length at most. */
  resume_bounds: ResumeRules;
  /** What is done with a film of wide gamut colour. */
  wide_gamut: WideGamutChoice;
  wide_gamut_choices: WideGamutChoice[];
  /** The languages the library really holds, which is what a picker offers. */
  audio_languages: string[];
  subtitle_languages: string[];
}

/** How far the preparation of a film has got. */
export interface Preparation {
  /** starting, reading, producing or ready. */
  step: "starting" | "reading" | "producing" | "ready";
  /** Seconds of film on the disk that a player can actually read. */
  ready_seconds: number;
  /** How many seconds make a comfortable start, which near the end of a film
   *  is whatever is left of it. */
  wanted_seconds: number;
  /** How much of the first piece a player needs is written, from 0 to 1. */
  first_written: number;
  /** How hard the machine is working on this film, while it is working. */
  producing: Producing | null;
}

/** What the tool says it is doing this second. */
export interface Producing {
  /** Pictures a second it says it is writing. */
  pictures_a_second: number;
  /** The same work against real time. Below one and the picture will stop. */
  speed: number;
}

/** How the server is reached. */
export type AccessMode = "proxy" | "self_signed" | "provided" | "automatic";

/** How the server is reached, and what came of it. */
export interface AccessStatus {
  mode: AccessMode;
  certificate_path: string | null;
  private_key_path: string | null;
  redirect_to_https: boolean;
  public_names: string[];
  /** The certificate in use, when the server encrypts. */
  certificate: {
    names: string[];
    issuer: string;
    not_before: string;
    not_after: string;
  } | null;
  /** Why it does not, when it should, as a word. */
  problem: string | null;
}

/** One line of the activity journal. What it says beyond its columns is in
 *  `details`, whose fields depend on `kind`. */
export interface ActivityLine {
  id: string;
  at: string;
  kind: string;
  level: "information" | "attention" | "trouble";
  user_id: string | null;
  /** Where the account's picture is served, when it has one. */
  user_avatar: string | null;
  work_id: string | null;
  /** What the browser said it was. */
  device: string | null;
  details: Record<string, unknown>;
}

export interface ActivityPage {
  lines: ActivityLine[];
  /** Whether there are older lines than these. */
  more: boolean;
}

/** The families the journal is read by. */
/** The families the journal is read by; "refused" is only the refused sign
    ins and held back accounts, for the page of security. */
export type ActivityFamily = "access" | "playback" | "library" | "server" | "refused";

/** One film playing on one device, as the administration follows it. */
export interface Watched {
  /** What a stop is asked of. */
  device: string;
  user: string;
  /** What the browser said it was when it signed in. */
  device_name: string;
  /** The browser its page found, when it could tell better than the line. */
  browser: string | null;
  work_id: string;
  title: string;
  kind: string;
  year: number | null;
  series: string | null;
  season: number | null;
  episode: number | null;
  /** For a song: who plays it, and its album. */
  artist: string | null;
  album: string | null;
  picture: string | null;
  /** Where the film had got to when the server sent this. */
  position_seconds: number;
  duration_seconds: number | null;
  started_at: string;
  paused: boolean;
  /** Asked to stop, and not stopped yet. */
  stopping: boolean;
  /** How hard the machine works on it, while a conversion runs. */
  producing: Producing | null;
  /** What was decided for it. Absent for a player heard before its plan,
   *  which is one carrying on across a restart of the server. */
  decision: WatchedDecision | null;
}

export interface WatchedDecision {
  /** direct_play, remux, transcode_audio or full_transcode. */
  method: string;
  expensive: boolean;
  reasons: { code: string; [key: string]: unknown }[];
  film: FilmHolds;
  rebuild: PictureRebuild | null;
  /** What the card is called when one rebuilds the picture. */
  card: string | null;
  /** Whether that card reads the film as well as writing it. */
  card_reads_the_film: boolean;
  sound: "copy" | "transcode" | "drop";
  subtitles: "none" | "external" | "burn_in";
  tone_map: boolean;
}

export interface BrowseOptions {
  library?: string;
  order?: string;
  descending?: boolean;
  after?: string | null;
  limit?: number;
  genre?: string;
  decade?: number;
  search?: string;
  unidentified?: boolean;
  /** One letter, or "#" for everything that starts with none. */
  initial?: string;
  /** Only what this account marked, which is a narrowing of the grid rather
      than a library of its own. */
  favourites?: boolean;
  /** Only what this account put aside to watch later, a narrowing too. */
  watchLater?: boolean;
  /** Only the libraries of one kind, whichever libraries those are. */
  kind?: LibraryKind;
}

/** Turns the choices of a grid into a query the server understands. */
export function browseQuery(options: BrowseOptions): string {
  const parameters = new URLSearchParams();
  if (options.library) parameters.set("library", options.library);
  if (options.order) parameters.set("order", options.order);
  if (options.descending) parameters.set("descending", "true");
  if (options.after) parameters.set("after", options.after);
  if (options.limit) parameters.set("limit", String(options.limit));
  if (options.genre) parameters.set("genre", options.genre);
  if (options.decade !== undefined) parameters.set("decade", String(options.decade));
  if (options.search) parameters.set("search", options.search);
  if (options.unidentified) parameters.set("unidentified", "true");
  if (options.initial) parameters.set("initial", options.initial);
  if (options.favourites) parameters.set("favourites", "true");
  if (options.watchLater) parameters.set("watch_later", "true");
  if (options.kind) parameters.set("kind", options.kind);
  return parameters.toString();
}

export const api = {
  system: (signal?: AbortSignal) => get<SystemInfo>("/api/v1/system/info", signal),
  overview: (signal?: AbortSignal) => get<Overview>("/api/v1/system/overview", signal),
  measures: (signal?: AbortSignal) => get<LiveMeasures>("/api/v1/system/measures", signal),
  measuresOver: (over: MeasuredOver, signal?: AbortSignal) =>
    get<MeasurePoint[]>(`/api/v1/system/measures/history?over=${over}`, signal),
  libraries: (signal?: AbortSignal) => get<Library[]>("/api/v1/libraries", signal),
  filters: (library: string, signal?: AbortSignal) =>
    get<Filters>(`/api/v1/libraries/${library}/filters`, signal),
  home: (library: string | undefined, signal?: AbortSignal) =>
    get<Home>(`/api/v1/home${library ? `?library=${library}` : ""}`, signal),
  works: (options: BrowseOptions, signal?: AbortSignal) =>
    get<Page>(`/api/v1/works?${browseQuery(options)}`, signal),
  work: (id: string, signal?: AbortSignal) => get<Work>(`/api/v1/works/${id}`, signal),
  person: (id: string, signal?: AbortSignal) => get<Person>(`/api/v1/people/${id}`, signal),
  jobs: (signal?: AbortSignal) => get<Jobs>("/api/v1/jobs", signal),
  /* The mode says how much to go over. Left out, the server does what it has
     always done, which is to fill in what is missing. */
  scan: (library: string, mode?: RefreshMode) =>
    post<{ job_id: string }>(
      `/api/v1/libraries/${library}/scan${mode ? `?mode=${mode}` : ""}`,
    ),
  identify: (library: string, mode?: RefreshMode) =>
    post<{ job_id: string }>(
      `/api/v1/libraries/${library}/identify${mode ? `?mode=${mode}` : ""}`,
    ),
  /* What a scan of one library does in one sitting. Both switches travel
     together, because they are one answer to one question on one screen. */
  setLibraryOptions: (library: string, options: {
    extract_subtitles: boolean;
    make_thumbnails: boolean;
    detect_openings: boolean;
    generate_subtitles: boolean;
    process_on_arrival: boolean;
    watch_in_real_time: boolean;
    keeps_resume_points: boolean;
    keeps_watched_marks: boolean;
    metadata_language: string;
  }) =>
    put<{
      extract_subtitles: boolean;
      make_thumbnails: boolean;
      detect_openings: boolean;
      generate_subtitles: boolean;
      process_on_arrival: boolean;
      watch_in_real_time: boolean;
      keeps_resume_points: boolean;
      keeps_watched_marks: boolean;
      metadata_language: string;
      changed: boolean;
      /** How many films went back in the queue, when the language changed. */
      asked_about_again: number | null;
    }>(`/api/v1/libraries/${library}/options`, options),
  /* The six scheduled tasks: when each runs by itself, what it has waiting,
     how it last went. Saying when one runs answers every task, as the screen
     draws them. */
  tasks: (signal?: AbortSignal) => get<ScheduledTasks>("/api/v1/tasks", signal),
  scheduleTask: (task: TaskName, runs_on_schedule: boolean, at_utc_minutes: number) =>
    put<ScheduledTasks>(`/api/v1/tasks/${task}/schedule`, { runs_on_schedule, at_utc_minutes }),
  runTask: (task: TaskName) => post<{ started: boolean }>(`/api/v1/tasks/${task}/run`),
  runEveryTask: () => post<{ started: boolean }>("/api/v1/tasks/run"),
  /* What the server does with a library. Everything travels together, because
     it is one screen and one answer, and what comes back is what was kept:
     a shape that cannot hold a thumbnail is brought into range rather than
     refused. */
  /* The folders inside one folder of the server's disk. Nothing means the top
     of the tree, which is where somebody with nothing typed in starts. */
  folders: (path: string | null, signal?: AbortSignal) =>
    get<Listing>(
      `/api/v1/folders${path ? `?path=${encodeURIComponent(path)}` : ""}`,
      signal,
    ),
  /* Declaring a library, and the two ways of putting one right afterwards. */
  createLibrary: (library: {
    name: string;
    kind: string;
    metadata_language: string;
    roots: string[];
    options: LibraryChoices;
  }) => post<{ id: string; name: string; scanning: boolean }>("/api/v1/libraries", library),
  renameLibrary: (library: string, name: string) =>
    put<{ name: string }>(`/api/v1/libraries/${library}/name`, { name }),
  addRoot: (library: string, path: string) =>
    post<{ id: string; label: string }>(`/api/v1/libraries/${library}/roots`, { path }),
  renameRoot: (library: string, root: string, label: string) =>
    put<{ id: string; label: string }>(
      `/api/v1/libraries/${library}/roots/${root}/label`,
      { label },
    ),
  /* Taking one away. The count is asked for the moment the question is put,
     rather than read off a listing that may be an hour old: the number
     somebody says yes to has to be the number that goes. No file on the disk
     is touched by any of these. */
  whatRemovingTakes: (library: string, signal?: AbortSignal) =>
    get<WouldGo>(`/api/v1/libraries/${library}/removal`, signal),
  removeLibrary: (library: string) => remove<WouldGo>(`/api/v1/libraries/${library}`),
  setAsideFiles: (library: string, signal?: AbortSignal) =>
    get<SetAsideFile[]>(`/api/v1/libraries/${library}/set-aside`, signal),
  /* Nothing named takes every one of them back. The library is scanned
     straight after, so they come back without anybody asking for it. */
  takeBackSetAside: (library: string, files: SetAsideFile[] | null) =>
    post<{ files: number }>(`/api/v1/libraries/${library}/set-aside/take-back`, {
      files: files?.map(({ root, relative_path }) => ({ root, relative_path })) ?? null,
    }),
  whatDeletingTakes: (works: string[], signal?: AbortSignal) =>
    post<Deletion>("/api/v1/deletion/what-it-takes", { works }, signal),
  deleteWorks: (works: string[], fromDisk: boolean) =>
    post<Deleted>("/api/v1/deletion", { works, from_disk: fromDisk }),
  whatRemovingAFolderTakes: (library: string, root: string, signal?: AbortSignal) =>
    get<WouldGo>(`/api/v1/libraries/${library}/roots/${root}/removal`, signal),
  removeRoot: (library: string, root: string) =>
    remove<WouldGo>(`/api/v1/libraries/${library}/roots/${root}`),
  libraryWork: (signal?: AbortSignal) =>
    get<LibraryWork>("/api/v1/settings/libraries", signal),
  setLibraryWork: (work: LibraryWork) =>
    put<LibraryWork>("/api/v1/settings/libraries", work),
  ratingsSettings: (signal?: AbortSignal) =>
    get<RatingsSettings>("/api/v1/settings/ratings", signal),
  setOmdbKey: (omdb_key: string) =>
    put<RatingsSettings>("/api/v1/settings/ratings", { omdb_key }),
  forgetOmdbKey: () => remove<RatingsSettings>("/api/v1/settings/ratings"),
  /* Subtitles from OpenSubtitles, for an administrator: the key and
     account, the tracks of a copy, what is offered for it, and downloading
     or taking away one. */
  speech: (signal?: AbortSignal) => get<SpeechStatus>("/api/v1/system/speech", signal),
  chooseSpeechModel: (model: string | null) =>
    put<SpeechStatus>("/api/v1/system/speech", { model }),
  downloadTranslationModel: () =>
    post<{ job_id: string }>("/api/v1/system/speech/translation/download"),
  forgetTranslationModel: () => remove<SpeechStatus>("/api/v1/system/speech/translation"),
  setSpeechEffort: (effort: SpeechEffort) =>
    put<SpeechStatus>("/api/v1/system/speech/effort", { effort }),
  downloadSpeechModel: (id: string) =>
    post<{ job_id: string }>(`/api/v1/system/speech/models/${id}/download`),
  forgetSpeechModel: (id: string) =>
    remove<SpeechStatus>(`/api/v1/system/speech/models/${id}`),
  openSubtitles: (signal?: AbortSignal) =>
    get<OpenSubtitlesSettings>("/api/v1/settings/opensubtitles", signal),
  setOpenSubtitles: (key: string, username: string, password: string) =>
    put<OpenSubtitlesSettings>("/api/v1/settings/opensubtitles", { key, username, password }),
  forgetOpenSubtitles: () => remove<OpenSubtitlesSettings>("/api/v1/settings/opensubtitles"),
  /** The thumbnails of a copy, without preparing it to be played. Refused
   *  while the copy has not been read for them. */
  thumbnailsOf: (source: string, signal?: AbortSignal) =>
    get<PlaybackThumbnails>(`/api/v1/playback/${source}/thumbnails`, signal),
  subtitleTracks: (source: string, signal?: AbortSignal) =>
    get<SubtitleTrackInfo[]>(`/api/v1/playback/${source}/subtitles`, signal),
  subtitleOffers: (source: string, languages: string[], signal?: AbortSignal) =>
    get<SubtitleOffer[]>(
      `/api/v1/playback/${source}/subtitles/online?languages=${encodeURIComponent(languages.join(","))}`,
      signal,
    ),
  downloadSubtitle: (source: string, offer: SubtitleOffer) =>
    post<{ track_id: string; remaining: number | null; ends_after_the_film: boolean }>(
      `/api/v1/playback/${source}/subtitles/online`,
      offer,
    ),
  removeSubtitle: (source: string, track: string) =>
    remove<null>(`/api/v1/playback/${source}/subtitles/online/${track}`),
  playbackSettings: (signal?: AbortSignal) =>
    get<PlaybackSettings>("/api/v1/settings/playback", signal),
  setPlaybackSettings: (settings: PlaybackSettings) =>
    put<PlaybackSettings>("/api/v1/settings/playback", settings),
  cardChoice: (signal?: AbortSignal) =>
    get<CardChoice>("/api/v1/settings/card", signal),
  chooseCard: (choice: CardChoice) =>
    put<CardChoice>("/api/v1/settings/card", {
      chosen: choice.chosen,
      other_card_when_refused: choice.other_card_when_refused,
      processor_when_refused: choice.processor_when_refused,
    }),
  cancelJob: (id: string) => post<{ stopped: boolean }>(`/api/v1/jobs/${id}/cancel`),
  forgetFinishedJobs: () => remove<{ forgotten: number }>("/api/v1/jobs/finished"),
  /* For the films the rules could not name: what a person could have meant,
     and the one they say it is.

     Every criterion narrows, and an identifier settles it outright: given
     one, the work it names is the only answer. An empty name is the film's
     own, which is what the field is filled with to begin with. */
  candidates: (work: string, asked: SearchCriteria, signal?: AbortSignal) => {
    const said = new URLSearchParams();
    if (asked.name) said.set("query", asked.name);
    if (asked.year) said.set("year", asked.year);
    if (asked.imdbId) said.set("imdb_id", asked.imdbId);
    if (asked.providerId) said.set("provider_id", asked.providerId);
    return get<Candidate[]>(`/api/v1/works/${work}/candidates?${said}`, signal);
  },
  /* Takes away what any provider said about a work, which is then left out
     of the automatic look up; and asks the provider again about one work,
     keeping every field written by hand. */
  /** Where a copy is handed over as a file to keep, for an account allowed
   *  to download. */
  downloadAddress: (source: string) => `/api/v1/playback/${source}/download`,
  forgetIdentity: (work: string) =>
    remove<{ identified: boolean }>(`/api/v1/works/${work}/identify`),
  refreshWork: (work: string) =>
    post<{ outcome: "described" | "not_found" | "postponed" }>(`/api/v1/works/${work}/refresh`),
  identifyByHand: (work: string, externalId: string, replacePictures = true) =>
    post<{ identified: boolean }>(`/api/v1/works/${work}/identify`, {
      external_id: externalId,
      replace_pictures: replacePictures,
    }),
  /* The server's collections: listed and opened by everybody, made and
     filled by whoever may manage them. */
  collections: (signal?: AbortSignal) => get<CollectionSummary[]>("/api/v1/collections", signal),
  collection: (id: string, signal?: AbortSignal) =>
    get<Collection>(`/api/v1/collections/${id}`, signal),
  createCollection: (name: string, works: string[]) =>
    post<{ id: string }>("/api/v1/collections", { name, works }),
  renameCollection: (id: string, name: string) =>
    put<null>(`/api/v1/collections/${id}/name`, { name }),
  deleteCollection: (id: string) => remove<null>(`/api/v1/collections/${id}`),
  putInCollection: (id: string, works: string[], in_it: boolean) =>
    post<null>(`/api/v1/collections/${id}/works`, { works, in_it }),
  collectionsHolding: (work: string, signal?: AbortSignal) =>
    get<{ collections: string[] }>(`/api/v1/works/${work}/collections`, signal),
  /* This account's playlists, which nobody else sees. */
  playlists: (signal?: AbortSignal) => get<PlaylistSummary[]>("/api/v1/playlists", signal),
  playlist: (id: string, signal?: AbortSignal) => get<Playlist>(`/api/v1/playlists/${id}`, signal),
  createPlaylist: (name: string, works: string[]) =>
    post<{ id: string }>("/api/v1/playlists", { name, works }),
  renamePlaylist: (id: string, name: string) => put<null>(`/api/v1/playlists/${id}/name`, { name }),
  deletePlaylist: (id: string) => remove<null>(`/api/v1/playlists/${id}`),
  putInPlaylist: (id: string, works: string[], in_it: boolean) =>
    post<null>(`/api/v1/playlists/${id}/works`, { works, in_it }),
  reorderPlaylist: (id: string, works: string[]) =>
    put<null>(`/api/v1/playlists/${id}/order`, { works }),
  playlistsHolding: (work: string, signal?: AbortSignal) =>
    get<{ playlists: string[] }>(`/api/v1/works/${work}/playlists`, signal),
  /* A work's details written by hand; each field locked stays as written
     whatever a later look up says. */
  details: (work: string, signal?: AbortSignal) =>
    get<WrittenDetails>(`/api/v1/works/${work}/details`, signal),
  writeDetails: (work: string, details: WrittenDetails) =>
    put<WrittenDetails>(`/api/v1/works/${work}/details`, details),
  /* The pictures a work wears, what the provider offers instead, and saying
     which one it is to wear. A picture chosen or taken off by hand is
     remembered as a choice and no later run undoes it. */
  heldPictures: (work: string, signal?: AbortSignal) =>
    get<HeldPicture[]>(`/api/v1/works/${work}/pictures`, signal),
  offeredPictures: (work: string, signal?: AbortSignal) =>
    get<OfferedPicture[]>(`/api/v1/works/${work}/pictures/offered`, signal),
  choosePicture: (work: string, kind: PictureKind, path: string) =>
    put<{ changed: boolean }>(`/api/v1/works/${work}/pictures/${kind}`, { path }),
  forgetPicture: (work: string, kind: PictureKind) =>
    remove<{ changed: boolean }>(`/api/v1/works/${work}/pictures/${kind}`),
  /* When two copies on one film turn out not to be the same film at all. */
  detachCopy: (copy: string) =>
    post<{ work_id: string }>(`/api/v1/copies/${copy}/detach`),
  /* Reads one file again for what it says about itself. A scan opens only
     what changed on disk, so this is the only way to reach a file the server
     has already described. */
  readCopyAgain: (copy: string) =>
    post<{ job_id: string }>(`/api/v1/copies/${copy}/read-again`),
  /* Everything worth asking about this installation, in one block of text
     rendered by the server so that it says exactly what the command line
     says. */
  report: (signal?: AbortSignal) => getText("/api/v1/system/diagnostics/text", signal),
  /* What the server has been saying, narrowed to the tags somebody ticked.
     The whole point is that one person can say "send me playback and
     subtitles" and the other sends exactly that. */
  journal: (query: JournalQuery, signal?: AbortSignal) =>
    get<Journal>(`/api/v1/system/journal?${journalQuery(query)}`, signal),
  journalText: (query: JournalQuery, signal?: AbortSignal) =>
    getText(`/api/v1/system/journal/text?${journalQuery(query)}`, signal),
  forgetJournal: () => remove<{ forgotten: number }>("/api/v1/system/journal"),
  /* One fact the page saw, for the journal. The server names the facts and
     refuses anything else, so this cannot become a way of writing whatever
     into it: see the `page` module on the server for the whole list. */
  tellTheJournal: (said: PageSaw) =>
    post<{ written: boolean }>("/api/v1/system/journal/page", said),
  /* For trying the slow path again: a subtitle already converted is served in
     a millisecond and proves nothing about the minute it took to get there. */
  forgetConvertedSubtitles: () =>
    remove<{ forgotten: number }>("/api/v1/system/cache/subtitles"),
  rememberTracks: (body: {
    work_id: string;
    source_id: string;
    audio_track_id: string | null;
    subtitle_track_id: string | null;
  }) => post<{ remembered: boolean }>("/api/v1/playback/tracks", body),
  /* The activity journal, newest first, of the families asked for or of all
     of them, after the last line already shown when one is given. */
  activity: (
    families: ActivityFamily[],
    before: string | null,
    signal?: AbortSignal,
    most?: number,
  ) => {
    const asked = new URLSearchParams();
    if (families.length > 0) asked.set("families", families.join(","));
    if (before) asked.set("before", before);
    if (most) asked.set("most", String(most));
    return get<ActivityPage>(`/api/v1/system/activity?${asked.toString()}`, signal);
  },
  access: (signal?: AbortSignal) => get<AccessStatus>("/api/v1/system/security/access", signal),
  chooseAccess: (mode: AccessMode, certificatePath: string | null, privateKeyPath: string | null) =>
    put<AccessStatus>("/api/v1/system/security/access", {
      mode,
      certificate_path: certificatePath,
      private_key_path: privateKeyPath,
    }),
  setAccessOptions: (redirectToHttps: boolean, publicNames: string[]) =>
    put<AccessStatus>("/api/v1/system/security/access/options", {
      redirect_to_https: redirectToHttps,
      public_names: publicNames.join(","),
    }),
  signInTries: (signal?: AbortSignal) =>
    get<{ tries: number }>("/api/v1/system/security/tries", signal),
  setSignInTries: (tries: number) =>
    put<{ tries: number }>("/api/v1/system/security/tries", { tries }),
  activityKeptDays: (signal?: AbortSignal) =>
    get<{ days: number }>("/api/v1/system/activity/kept", signal),
  keepActivityDays: (days: number) =>
    put<{ days: number }>("/api/v1/system/activity/kept", { days }),
  /* What the server is called and the logo it wears, for the administrator
     changing them. Each change answers both as the server now holds them. */
  server: (signal?: AbortSignal) => get<ServerSettings>("/api/v1/settings/server", signal),
  renameServer: (server_name: string) =>
    put<ServerSettings>("/api/v1/settings/server/name", { server_name }),
  forgetServerName: () => remove<ServerSettings>("/api/v1/settings/server/name"),
  setServerLogo: (image: Blob) => put<ServerSettings>("/api/v1/settings/server/logo", image),
  removeServerLogo: () => remove<ServerSettings>("/api/v1/settings/server/logo"),
  /* What stands behind the sign in screen: a drawn background, and a picture
     that wins over it while it is there. */
  setDoorBackground: (door_background: LoginBackgroundStyle) =>
    put<ServerSettings>("/api/v1/settings/server/door/background", { door_background }),
  setDoorPicture: (image: Blob) =>
    put<ServerSettings>("/api/v1/settings/server/door/picture", image),
  setDefaultTheme: (default_theme: ServerTheme) =>
    put<ServerSettings>("/api/v1/settings/server/theme", { default_theme }),
  setDoorSlogan: (door_slogan: string) =>
    put<ServerSettings>("/api/v1/settings/server/door/slogan", { door_slogan }),
  removeDoorPicture: () => remove<ServerSettings>("/api/v1/settings/server/door/picture"),
  /* The live line of every page: what changes in this account's
     notifications and, for an administrator, "activity" whenever a line of
     the journal is written and, when asked for, "playing" with what is being
     watched whenever it changes, and "failed" when it could not be read. */
  liveLine: (playing: boolean) =>
    new EventSource(`/api/v1/system/live${playing ? "?playing=true" : ""}`),
  /* The player is told on its next word, and the server closes the
     conversion itself if it never obeys. */
  stopPlaying: (device: string) =>
    post<{ stopping: boolean }>(`/api/v1/system/playing/${device}/stop`),
  plan: (source: string, body: unknown, signal?: AbortSignal) =>
    post<PlaybackPlan>(`/api/v1/playback/${source}/plan`, body, signal),
  /**
   * Marks a film as one this viewer likes, or takes the mark off.
   *
   * Answers what it is now rather than what was asked for, so a button pressed
   * twice in a second cannot end up saying one thing while the server says
   * another.
   */
  /** What the server calls itself and the mark it was given. Answered without
   *  an account, which is what lets the door carry them. */
  branding: (signal?: AbortSignal) => get<Branding>("/api/v1/public/branding", signal),
  /* The names this server offers on its sign in screen, which is the one
     thing it says to somebody who has not signed in. Empty when the server
     was told not to offer them, when everybody asked to be left off, and on a
     brand new server that has no accounts yet: the screen draws no row rather
     than telling the three apart. */
  namesAtTheDoor: (signal?: AbortSignal) =>
    get<{ names: NameAtTheDoor[] }>("/api/v1/public/names", signal),
  /* The door. Signing in answers the account, which is what the interface is
     drawn from; the session itself travels in a cookie the browser keeps and
     this interface never sees.

     Remembering is the one thing about that cookie somebody chooses: said
     yes, the browser keeps it and the machine stays signed in; said no, the
     browser drops it the moment it closes. The client is what this browser
     calls itself, so a session it already held for the account is replaced. */
  signIn: (name: string, password: string, remember: boolean, client: string) =>
    post<Account>("/api/v1/session", { name, password, remember, client }),
  signOut: () => remove<{ signed_out: boolean }>("/api/v1/session"),
  me: (signal?: AbortSignal) => get<Account>("/api/v1/me", signal),
  /* The first account of a brand new server, which is an administrator and is
     signed in straight away. Refused once there is one. */
  setUp: (name: string, password: string, remember: boolean, client: string, language: string) =>
    post<Account>("/api/v1/setup", { name, password, remember, client, language }),
  /* What an administrator is taken through once, after the first account. */
  firstSteps: (signal?: AbortSignal) =>
    get<{ pending: boolean }>("/api/v1/setup/first-steps", signal),
  finishFirstSteps: () => post<{ pending: boolean }>("/api/v1/setup/first-steps/done"),
  /* The picture of the account signed in, sent as the file chosen. Both
     answer the account as it now is. */
  setAvatar: (image: Blob) => put<Account>("/api/v1/me/avatar", image),
  removeAvatar: () => remove<Account>("/api/v1/me/avatar"),
  /* Changing it signs every other device out and keeps this one going. */
  changePassword: (current: string, wanted: string) =>
    put<Account>("/api/v1/me/password", { current, wanted }),
  /* Answers the account under its new name; every device stays signed in. */
  rename: (name: string) => put<Account>("/api/v1/me/name", { name }),
  accounts: (signal?: AbortSignal) => get<ManagedAccount[]>("/api/v1/accounts", signal),
  createAccount: (name: string, password: string, rights: Rights) =>
    post<ManagedAccount>("/api/v1/accounts", { name, password, rights }),
  setRights: (account: string, rights: Rights) =>
    put<ManagedAccount>(`/api/v1/accounts/${account}/rights`, rights),
  renameAccount: (account: string, name: string) =>
    put<ManagedAccount>(`/api/v1/accounts/${account}/name`, { name }),
  putPassword: (account: string, password: string) =>
    put<{ changed: boolean }>(`/api/v1/accounts/${account}/password`, { password }),
  signOutEverywhere: (account: string) =>
    remove<{ signed_out: number }>(`/api/v1/accounts/${account}/sessions`),
  removeAccount: (account: string) =>
    remove<{ removed: boolean }>(`/api/v1/accounts/${account}`),
  /* Every device of every account, for the administration; one's own for
     everybody. Signing one out stops whatever it was playing. */
  devices: (signal?: AbortSignal) => get<SignedInDevice[]>("/api/v1/devices", signal),
  signOutDevice: (device: string) =>
    remove<{ signed_out: boolean }>(`/api/v1/devices/${device}`),
  myDevices: (signal?: AbortSignal) => get<SignedInDevice[]>("/api/v1/me/devices", signal),
  signOutMyDevice: (device: string) =>
    remove<{ signed_out: boolean }>(`/api/v1/me/devices/${device}`),
  /* Every one of one's own devices but this one; answers how many. */
  signOutMyOtherDevices: () => remove<{ signed_out: number }>("/api/v1/me/devices"),
  setFavourite: (work: string, favourite: boolean) =>
    put<{ favourite: boolean }>(`/api/v1/works/${work}/favourite`, { favourite }),
  /* Where a person says a file's opening and closing titles really are,
     for an administrator. Both answer what a player now offers to skip. */
  correctSegment: (source: string, kind: string, correction: SegmentCorrection) =>
    put<Stretches>(`/api/v1/playback/${source}/segments/${kind}`, correction),
  takeBackSegment: (source: string, kind: string) =>
    remove<Stretches>(`/api/v1/playback/${source}/segments/${kind}`),
  setWatchLater: (work: string, later: boolean) =>
    put<{ watch_later: boolean }>(`/api/v1/works/${work}/watch-later`, { watch_later: later }),
  /* On a season or a series this marks every episode below it, which is what
     the answer is about: the tick is drawn from what came back. */
  setWatched: (work: string, watched: boolean) =>
    put<{ watched: boolean }>(`/api/v1/works/${work}/watched`, { watched }),
  /* The server's own shelf, not a bookmark: an administrator's to set, and
     refused to anybody else. */
  setPinned: (work: string, pinned: boolean) =>
    put<{ pinned: boolean; displaced: string[] }>(`/api/v1/works/${work}/pinned`, { pinned }),
  openSession: (source: string, body: unknown, signal?: AbortSignal) =>
    post<PlaybackSession>(`/api/v1/playback/${source}/session`, body, signal),
  preparation: (session: string, signal?: AbortSignal) =>
    get<Preparation>(`/api/v1/stream/${session}/preparation`, signal),
  preferences: (signal?: AbortSignal) =>
    get<ViewerPreferences>("/api/v1/preferences", signal),
  savePreferences: (changes: Partial<ViewerPreferences>, signal?: AbortSignal) =>
    put<ViewerPreferences>("/api/v1/preferences", changes, signal),
  /**
   * Closes a session, including while the page is going away.
   *
   * A request started as a tab closes is normally dropped, and a dropped one
   * here means a media tool converting a film nobody is watching until the
   * server notices on its own. Kept alive, the browser delivers it anyway.
   */
  /**
   * Says that somebody still has this film open, though they are asking for
   * nothing.
   *
   * Answers whether the session is still there. A film paused asks the server
   * for nothing at all, and a session nobody asks anything of is swept away:
   * without this, pausing long enough loses the film where the viewer left
   * it.
   */
  stillWatching: (session: string) =>
    fetch(`/api/v1/stream/${session}`, { method: "POST" }).then((answer) => answer.ok),
  closeSession: (session: string) => {
    void fetch(`/api/v1/stream/${session}`, { method: "DELETE", keepalive: true }).catch(() => {
      // A session that could not be closed is swept by the server once
      // nobody has asked it for anything, so there is nothing to say here.
    });
  },
  /**
   * The same, handed to the browser to deliver while the page goes away.
   *
   * A request started as a tab closes is usually dropped; this one is not,
   * which is what keeps the position of a film someone simply closed.
   */
  reportPositionOnTheWayOut: (work: string, seconds: number) =>
    handOver("/api/v1/playback/progress", {
      work_id: work,
      position_seconds: seconds,
      reported_at: new Date().toISOString(),
      leaving: true,
    }),
  /* Also how the server knows where the film is and whether it stands
     still, so the answer says whether an administrator asked for it to stop.
     `leaving` is the last word of a player leaving it. */
  reportPosition: (work: string, seconds: number, paused: boolean, leaving = false) =>
    post<{ kept: boolean; stop: boolean }>("/api/v1/playback/progress", {
      work_id: work,
      position_seconds: seconds,
      // The instant this client measured it. A report that arrives after a
      // fresher one is refused, so coming back online cannot undo progress
      // made elsewhere in the meantime.
      reported_at: new Date().toISOString(),
      paused,
      leaving,
    }),
  /* The same news while the film is not anywhere worth remembering yet:
     getting ready, or its first seconds. Null until the picture has shown
     anything. */
  stillPlaying: (
    work: string,
    seconds: number | null,
    paused: boolean,
    leaving = false,
    fresh = false,
  ) =>
    post<{ stop: boolean }>("/api/v1/playback/watching", {
      work_id: work,
      position_seconds: seconds,
      paused,
      leaving,
      fresh,
    }),
  /* Held open by the player while it shows the film: its end tells the
     server the player is gone, and the server sends "stop" down it the moment
     an administrator asks. */
  playerLine: (work: string) => new EventSource(`/api/v1/playback/watching/${work}/live`),
  /* Which browser this is, as the page found by asking it. */
  nameTheBrowser: (browser: string | null) =>
    put<{ named: boolean }>("/api/v1/me/browser", { browser }),
  stillPlayingOnTheWayOut: (work: string, seconds: number | null) =>
    handOver("/api/v1/playback/watching", {
      work_id: work,
      position_seconds: seconds,
      leaving: true,
    }),
  /* Where the clips a device is measured on stand on the server, and what
     they are once they are ready. */
  calibrationClips: () => get<CalibrationClips>("/api/v1/calibration/clips"),
  /* Asks the server to make whatever clips are missing. */
  prepareCalibrationClips: () =>
    post<{ preparing: boolean }>("/api/v1/calibration/clips"),
  /* One clip, whole, to be played from memory. */
  calibrationClip: async (url: string) => (await getRaw(url, "video/mp4")).blob(),
  /* Keeps this device's whole calibration. The server refuses one that does
     not answer for every codec it offered. */
  recordCalibration: (
    clientId: string,
    body: { calibration_version: number; codecs: CodecResult[] },
  ) => post<{ recorded: boolean }>(`/api/v1/calibration/${clientId}`, body),
  /* This device's calibration, or null when it has no whole one. */
  deviceCalibration: (clientId: string) =>
    get<DeviceCalibration | null>(`/api/v1/calibration/${clientId}`),
  /* Forgets this device's calibration. */
  forgetCalibration: (clientId: string) =>
    remove<{ forgotten: boolean }>(`/api/v1/calibration/${clientId}`),
};

/** One clip a device is measured on. */
export interface CalibrationClip {
  codec: string;
  height: number;
  frame_rate: number;
  url: string;
}

/** Where the clips stand on the server. */
export interface CalibrationClips {
  calibration_version: number;
  state: "not_prepared" | "preparing" | "failed" | "ready";
  done: number;
  total: number;
  reason: string | null;
  clips: CalibrationClip[];
}

/** One clip of one codec at one height, as this device played it. */
export interface Measurement {
  height: number;
  passed: boolean;
  dropped_share: number;
  shown_share: number;
}

/** What this device was found to do with one codec. */
export interface CodecResult {
  codec: string;
  /** The tallest picture it played cleanly, or null for none. */
  smooth_height: number | null;
  measurements: Measurement[];
}

/** One whole calibration of this device. */
export interface DeviceCalibration {
  calibration_version: number;
  measured_at: string;
  codecs: CodecResult[];
}

/**
 * The set of sizes a browser picks from, and the one to load by default.
 *
 * Handing over every size lets the browser choose by screen and by how much
 * room the picture actually has, which is what stops a phone downloading a
 * poster meant for a television.
 */
export function pictureSet(pictures: Picture[]): { src: string; srcSet: string } | null {
  if (pictures.length === 0) {
    return null;
  }
  const sized = pictures.filter((picture) => picture.width !== null);
  if (sized.length === 0) {
    return { src: pictures[0].url, srcSet: "" };
  }
  const smallest = sized.reduce((best, picture) =>
    (picture.width ?? 0) < (best.width ?? 0) ? picture : best,
  );
  return {
    src: smallest.url,
    srcSet: sized.map((picture) => `${picture.url} ${picture.width}w`).join(", "),
  };
}
