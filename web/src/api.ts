/*
 * The one place that talks to the server.
 *
 * Every shape here mirrors what the server sends, so a change on that side
 * fails at compilation rather than at three in the morning on a screen.
 */

export interface Picture {
  url: string;
  width: number | null;
  height: number | null;
}

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
  access: "missing" | "unreadable" | "read_only" | "read_write";
  explanation_code: string;
}

export interface Library {
  id: string;
  name: string;
  kind: string;
  works: number;
  version: number;
  roots: Root[];
}

export interface Filters {
  genres: { name: string; works: number }[];
  decades: { decade: number; works: number }[];
  /** The letters titles really start with, the bucket for the rest first. */
  initials: { name: string; works: number }[];
}

export interface Home {
  /** Films started and not finished, the latest first. */
  carry_on: (Card & { position_seconds: number })[];
  recently_added: Card[];
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
  genres: string[];
  studios: string[];
  collection: string | null;
  cast: Credit[];
  crew: Credit[];
  poster: Picture[];
  backdrop: Picture[];
  versions: Version[];
  trailers: Trailer[];
  external_ids: { provider: string; id: string }[];
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
    }
  | {
      session: string;
      saw: "playback_stalled";
      at_second: number;
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
    }
  | {
      session: string;
      saw: "playback_refused";
      /** Why the library gave up, in its own words. Cut short by the server. */
      because: string;
      /** Whether the film went on playing, read by the browser itself. */
      browser_took_over: boolean;
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
  ) {
    super(code);
  }
}

async function get<T>(path: string, signal?: AbortSignal): Promise<T> {
  let response: Response;
  try {
    response = await fetch(path, { signal, headers: { accept: "application/json" } });
  } catch (cause) {
    if (cause instanceof DOMException && cause.name === "AbortError") {
      throw cause;
    }
    throw new ApiError("unreachable", 0);
  }

  if (!response.ok) {
    const body = await response.json().catch(() => null);
    throw new ApiError(body?.code ?? "generic", response.status);
  }
  return (await response.json()) as T;
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
  let response: Response;
  try {
    response = await fetch(path, { signal, headers: { accept: "text/plain" } });
  } catch (cause) {
    if (cause instanceof DOMException && cause.name === "AbortError") {
      throw cause;
    }
    throw new ApiError("unreachable", 0);
  }

  if (!response.ok) {
    throw new ApiError("generic", response.status);
  }
  return response.text();
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
  let response: Response;
  try {
    response = await fetch(path, {
      method,
      headers:
        body === undefined
          ? { accept: "application/json" }
          : { accept: "application/json", "content-type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
      signal,
    });
  } catch {
    throw new ApiError("unreachable", 0);
  }
  if (!response.ok) {
    const body = await response.json().catch(() => null);
    throw new ApiError(body?.code ?? "generic", response.status);
  }
  return (await response.json()) as T;
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
  /** Segments on the disk that a player can actually read. */
  ready: number;
  /** How many make a comfortable start. */
  wanted: number;
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
  scan: (library: string) => post<{ job_id: string }>(`/api/v1/libraries/${library}/scan`),
  identify: (library: string) => post<{ job_id: string }>(`/api/v1/libraries/${library}/identify`),
  cancelJob: (id: string) => post<{ stopped: boolean }>(`/api/v1/jobs/${id}/cancel`),
  forgetFinishedJobs: () => remove<{ forgotten: number }>("/api/v1/jobs/finished"),
  /* For the films the rules could not name: what a person could have meant,
     and the one they say it is. */
  candidates: (work: string, query: string, signal?: AbortSignal) =>
    get<Candidate[]>(
      `/api/v1/works/${work}/candidates?query=${encodeURIComponent(query)}`,
      signal,
    ),
  identifyByHand: (work: string, externalId: string) =>
    post<{ identified: boolean }>(`/api/v1/works/${work}/identify`, {
      external_id: externalId,
    }),
  /* When two copies on one film turn out not to be the same film at all. */
  detachCopy: (copy: string) =>
    post<{ work_id: string }>(`/api/v1/copies/${copy}/detach`),
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
};

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
