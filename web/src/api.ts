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
  may_delete: boolean;
  may_delete_from_disk: boolean;
}

/**
 * What the door needs to draw itself, before anybody has signed in.
 *
 * The one thing this server says to somebody it does not know: its name, its
 * mark, and whether it has been set up at all. No account list, no library
 * name, no version.
 */
export interface Branding {
  server_name: string;
  logo_path: string | null;
  login_background_path: string | null;
  /** False on a brand new server, which asks for a first account instead of a
      password. */
  setup_complete: boolean;
}

export interface Picture {
  url: string;
  width: number | null;
  height: number | null;
}

export type WorkKind = "movie" | "series" | "season" | "episode";

/** What a library holds, which is what the header's categories are built
    from: a kind with no library of it is a category that does not exist. */
export type LibraryKind = "movies" | "series" | "anime" | "shows" | "music";

export type Seen = "not_started" | "in_progress" | "watched";

export interface Card {
  id: string;
  title: string;
  year: number | null;
  runtime_minutes: number | null;
  rating: number | null;
  identification: "pending" | "identified" | "unidentified" | "manual";
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
  /** Where they stopped, in seconds, only where they stopped partway. */
  resume_from_seconds: number | null;
  favourite: boolean;
  /** Episodes below, and how many are left. Both nothing for a film. */
  episodes: number;
  unwatched: number;
  /** The copy a play button on the card starts, when one is on disk. */
  source: string | null;
}

/** A film a person could have meant, as the provider describes it. */
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

export type IdentificationNote =
  | "no_match"
  | "provider_unreachable"
  | "provider_busy"
  | "provider_unreadable";

export interface Page {
  cards: Card[];
  next: string | null;
}

export interface Root {
  label: string;
  /** Its whole path on the server's disk. Shown only on the screen where
      roots are managed: the one place telling two of them apart by more than
      a label matters. */
  path: string;
  access: "missing" | "unreadable" | "read_only" | "read_write";
  explanation_code: string;
  /** What renaming this folder needs. */
  id: string;
}

export interface Library {
  id: string;
  name: string;
  kind: LibraryKind;
  works: number;
  version: number;
  /** Whether a scan of this library reads every film for where it can be
      started, rather than leaving it to the upkeep. */
  key_frames_during_scan: boolean;
  /** The same, for the pictures of the playback bar. */
  thumbnails_during_scan: boolean;
  /** The language this library's films are described in, as a two letter code.
      Changing it asks the provider about every film again. */
  metadata_language: string;
  roots: Root[];
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

/** One of the readings the upkeep is made of, on one library. */
export interface UpkeepTask {
  task: "key_frames" | "subtitles" | "thumbnails" | "openings";
  library: string;
  library_name: string;
  /** Films still waiting, or seasons when this one counts seasons. Nought
      means there is nothing to start. */
  waiting: number;
  done: number;
  /** Whether the two numbers above count seasons rather than files. Listening
      for the titles a season shares is done season by season. */
  counts_seasons: boolean;
  /** Whether the scan of that library does this reading itself. */
  during_the_scan: boolean;
  /** Whether it is running right now. */
  under_way: boolean;
  /** When it last ran to an end here, as an instant, or nothing when it never
      has. A reading that never ran and one that ran last night and found
      nothing look alike without it. */
  last_run: string | null;
  /** How that run ended, in the words every other job uses. */
  last_run_state: string | null;
  last_run_seconds: number | null;
}

export interface Upkeep {
  tasks: UpkeepTask[];
  /** When it next runs on its own, as an instant: shown in the hour of whoever
      reads it rather than in the server's. Absent when it never does. */
  next_run: string | null;
  settings: LibraryWork;
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
  thumbnails_enabled: boolean;
  thumbnails_every_seconds: number;
  thumbnails_height: number;
  thumbnails_columns: number;
  thumbnails_rows: number;
  upkeep_nightly: boolean;
  /** Minutes since midnight, **in UTC**. The server keeps the one clock it can
      read with certainty; this side turns it into the time of whoever looks. */
  upkeep_at_utc_minutes: number;
}

/**
 * What the server is set to do about wide gamut colour it cannot show a
 * client, for every viewer of this server rather than any one of them.
 */
export interface PlaybackSettings {
  /** Never convert such colour, even where a client cannot show it correctly.
      Off by default. Dolby Vision without a compatible base layer is
      converted regardless, since left alone it looks broken rather than
      merely washed out. */
  tone_mapping_disabled: boolean;
}

export interface Filters {
  genres: { name: string; works: number }[];
  decades: { decade: number; works: number }[];
  /** The letters titles really start with, the bucket for the rest first. */
  initials: { name: string; works: number }[];
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
  /** One row per kind of library this server really holds. */
  shelves: { kind: LibraryKind; cards: Card[]; picture: Picture[] }[];
  works: number;
  awaiting_identification: number;
}

export interface Credit {
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
  /** Where the file is, root included, and the disk it is on. */
  path: string;
  root_label: string;
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

/** One work hanging under another: a season of a series, an episode of a season. */
export interface Child {
  id: string;
  kind: "season" | "episode";
  /** The season number, the episode number. */
  number: number | null;
  /** The name it carries today. A page shows the number in the language it is
   *  being read in and keeps this for whatever the name adds to it. */
  title: string;
  runtime_minutes: number | null;
  /** How many episodes a season holds. Zero for an episode. */
  child_count: number;
  /** How many of those this viewer has left to watch. */
  unwatched: number;
  /** Whether this viewer has watched it. Only ever true of an episode. */
  watched: boolean;
  /** Where this viewer stopped in it, when they stopped partway. */
  resume_from_seconds: number | null;
  /** False when no file of it is on the disk, so a page says so rather than
   *  offering a button that fails when it is pressed. */
  playable: boolean;
  identification: Card["identification"];
  color: string | null;
  poster: Picture[];
  /** The biggest copy on disk, so a row of these can start one playing on its
   *  own. Absent along with `playable`. */
  source_id: string | null;
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
  /** The seasons of a series, the episodes of a season, in order. Empty for
   *  anything met on its own. */
  children: Child[];
  /** The way back up, nearest first. Empty for anything met on its own. */
  ancestry: Ancestor[];
  /** The episode to watch next: the first one left on a series or a season,
   *  the one after this on an episode. Absent when there is none. */
  carry_on_with: NextEpisode | null;
  /** The episode before this one, so the player can offer to step back into
   *  it. Absent for anything that is not an episode, and for the first
   *  episode of a series. */
  previous_episode: NextEpisode | null;
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

/**
 * A fact the page may tell the journal.
 *
 * The server defines the list and refuses anything outside it, so this type is
 * that list and never a message of the page's own making. Half of what happens
 * when a film starts happens here rather than on the server, and the journal
 * showed none of it.
 */
export type PageSaw =
  | {
      session: string;
      saw: "playback_began";
      /** What the playlist told the library, when it read it. */
      playlist_said_second: number | null;
      /** Where it settled on beginning. */
      began_at_second: number;
      /** The first segment it asked the server for. */
      first_segment: number;
    }
  | {
      session: string;
      saw: "viewer_jumped";
      from_second: number;
      to_second: number;
      was_playing: boolean;
      moved_by: HowItMoved;
    }
  | {
      session: string;
      saw: "the_picture_arrived";
      /** How wide and tall the browser says the picture is meant to be shown. */
      across: number;
      down: number;
      /** The box the page is drawing it in. */
      drawn_across: number;
      drawn_down: number;
    }
  | {
      session: string;
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
      session: string;
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
      session: string;
      saw: "playback_picked_up_again";
      at_second: number;
      waited_ms: number;
    }
  | {
      session: string;
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
      session: string;
      saw: "pictures_were_dropped";
      at_second: number;
      /** The stretch this counts over, in milliseconds. */
      over_ms: number;
      pictures_shown: number;
      pictures_dropped: number;
    }
  | {
      session: string;
      saw: "playback_refused";
      /** Why the library gave up, in its own words. Cut short by the server. */
      because: string;
      /** Whether the film went on playing, read by the browser itself. */
      browser_took_over: boolean;
    }
  | {
      session: string;
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
    };

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
      headers: body === undefined ? { accept } : { accept, "content-type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
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

async function get<T>(path: string, signal?: AbortSignal): Promise<T> {
  return (await exchange(path, "application/json", undefined, undefined, signal)).json() as Promise<T>;
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

const post = <T>(path: string, body?: unknown, signal?: AbortSignal) =>
  send<T>("POST", path, body, signal);
const remove = <T>(path: string, signal?: AbortSignal) =>
  send<T>("DELETE", path, undefined, signal);
const put = <T>(path: string, body?: unknown, signal?: AbortSignal) =>
  send<T>("PUT", path, body, signal);

async function send<T>(
  method: string,
  path: string,
  body?: unknown,
  signal?: AbortSignal,
): Promise<T> {
  return (await exchange(path, "application/json", method, body, signal)).json() as Promise<T>;
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
  /** Whether this viewer has marked the film as one they like. */
  favourite: boolean;
  /** What the file itself holds, beside what is being made of it. */
  film: FilmHolds;
}

/** One stretch of a film a button offers to skip. */
export interface PlaybackSegment {
  /** recap, intro, outro or advertisement. */
  kind: string;
  from_second: number;
  to_second: number;
}

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
  downmix_method: string;
  downmix_gain: number;
  /** The range the gain is kept inside, so a slider cannot be dragged
   *  somewhere the server would refuse. */
  downmix_gain_range: [number, number];
  downmix_methods: string[];
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
  if (options.kind) parameters.set("kind", options.kind);
  return parameters.toString();
}

export const api = {
  system: (signal?: AbortSignal) => get<SystemInfo>("/api/v1/system/info", signal),
  libraries: (signal?: AbortSignal) => get<Library[]>("/api/v1/libraries", signal),
  filters: (library: string, signal?: AbortSignal) =>
    get<Filters>(`/api/v1/libraries/${library}/filters`, signal),
  home: (library: string | undefined, signal?: AbortSignal) =>
    get<Home>(`/api/v1/home${library ? `?library=${library}` : ""}`, signal),
  works: (options: BrowseOptions, signal?: AbortSignal) =>
    get<Page>(`/api/v1/works?${browseQuery(options)}`, signal),
  work: (id: string, signal?: AbortSignal) => get<Work>(`/api/v1/works/${id}`, signal),
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
    key_frames_during_scan: boolean;
    thumbnails_during_scan: boolean;
    metadata_language: string;
  }) =>
    put<{
      key_frames_during_scan: boolean;
      thumbnails_during_scan: boolean;
      metadata_language: string;
      changed: boolean;
      /** How many films went back in the queue, when the language changed. */
      asked_about_again: number | null;
    }>(`/api/v1/libraries/${library}/options`, options),
  /* The two readings that go through every film, what each has left, and when
     the server will next do them on its own. */
  upkeep: (signal?: AbortSignal) => get<Upkeep>("/api/v1/upkeep", signal),
  runUpkeep: () => post<{ started: number }>("/api/v1/upkeep/run"),
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
  whatRemovingAFolderTakes: (library: string, root: string, signal?: AbortSignal) =>
    get<WouldGo>(`/api/v1/libraries/${library}/roots/${root}/removal`, signal),
  removeRoot: (library: string, root: string) =>
    remove<WouldGo>(`/api/v1/libraries/${library}/roots/${root}`),
  libraryWork: (signal?: AbortSignal) =>
    get<LibraryWork>("/api/v1/settings/libraries", signal),
  setLibraryWork: (work: LibraryWork) =>
    put<LibraryWork>("/api/v1/settings/libraries", work),
  playbackSettings: (signal?: AbortSignal) =>
    get<PlaybackSettings>("/api/v1/settings/playback", signal),
  setPlaybackSettings: (settings: PlaybackSettings) =>
    put<PlaybackSettings>("/api/v1/settings/playback", settings),
  runUpkeepTask: (library: string, task: string) =>
    post<{ job_id: string }>(`/api/v1/libraries/${library}/upkeep/${task}`),
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
  identifyByHand: (work: string, externalId: string, replacePictures = true) =>
    post<{ identified: boolean }>(`/api/v1/works/${work}/identify`, {
      external_id: externalId,
      replace_pictures: replacePictures,
    }),
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
  /* The door. Signing in answers the account, which is what the interface is
     drawn from; the session itself travels in a cookie the browser keeps and
     this interface never sees. */
  signIn: (name: string, password: string) =>
    post<Account>("/api/v1/session", { name, password }),
  signOut: () => remove<{ signed_out: boolean }>("/api/v1/session"),
  me: (signal?: AbortSignal) => get<Account>("/api/v1/me", signal),
  /* The first account of a brand new server, which is an administrator and is
     signed in straight away. Refused once there is one. */
  setUp: (name: string, password: string) =>
    post<Account>("/api/v1/setup", { name, password }),
  /* Changing it signs every other device out and keeps this one going. */
  changePassword: (current: string, wanted: string) =>
    put<Account>("/api/v1/me/password", { current, wanted }),
  setFavourite: (work: string, favourite: boolean) =>
    put<{ favourite: boolean }>(`/api/v1/works/${work}/favourite`, { favourite }),
  /* On a season or a series this marks every episode below it, which is what
     the answer is about: the tick is drawn from what came back. */
  setWatched: (work: string, watched: boolean) =>
    put<{ watched: boolean }>(`/api/v1/works/${work}/watched`, { watched }),
  /* The server's own shelf, not a bookmark: an administrator's to set, and
     refused to anybody else. */
  setPinned: (work: string, pinned: boolean) =>
    put<{ pinned: boolean }>(`/api/v1/works/${work}/pinned`, { pinned }),
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
  reportPositionOnTheWayOut: (work: string, seconds: number) => {
    const body = JSON.stringify({
      work_id: work,
      position_seconds: seconds,
      reported_at: new Date().toISOString(),
    });
    navigator.sendBeacon?.(
      "/api/v1/playback/progress",
      new Blob([body], { type: "application/json" }),
    );
  },
  reportPosition: (work: string, seconds: number) =>
    post<{ kept: boolean }>("/api/v1/playback/progress", {
      work_id: work,
      position_seconds: seconds,
      // The instant this client measured it. A report that arrives after a
      // fresher one is refused, so coming back online cannot undo progress
      // made elsewhere in the meantime.
      reported_at: new Date().toISOString(),
    }),
  /* Opens a session against the reference film, rebuilt into one codec at one
     height, for a calibration to watch and measure. Nameless: the same
     question for whoever asks it. */
  openCalibrationSession: (codec: string, height: number) =>
    /* The height and the rate come back because they are the server's answer
       and not the question: a film is never asked to be taller than it is,
       and keeping up is counted against the rate it really runs at. */
    post<{ id: string; playlist_url: string; height: number; frame_rate: number }>(
      "/api/v1/calibration/session",
      { codec, height },
    ),
  /* Records what this device measured for one codec, under its own
     identifier and nothing else. */
  recordCalibration: (body: {
    client_id: string;
    codec: string;
    calibration_version: number;
    usable: boolean;
    tested_height: number;
    dropped_share: number;
    shown_share: number;
    found_by: "test" | "watching";
  }) => post<{ recorded: boolean }>("/api/v1/calibration/verdict", body),
  /* Everything measured for this device so far, one entry per codec. */
  calibrationProfile: (clientId: string) =>
    get<CalibrationEntry[]>(`/api/v1/calibration/${clientId}`),
  /* Forgets everything measured for this device, all codecs at once. */
  forgetCalibration: (clientId: string) =>
    remove<{ forgotten: boolean }>(`/api/v1/calibration/${clientId}`),
};

/** What was measured for one codec, on this device. */
export interface CalibrationEntry {
  codec: string;
  calibration_version: number;
  usable: boolean;
  tested_height: number;
  dropped_share: number;
  /* The share of the pictures the film asked for that ever appeared at all:
     the half of the answer a dropped share alone never catches, since a
     decoder too slow to make pictures throws none of them away. */
  shown_share: number;
  /* Whether this came out of the test or out of watching a real film. A real
     film is the stronger of the two, and the one a test never overrules. */
  found_by: "test" | "watching";
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
