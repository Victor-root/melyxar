/*
 * What the server says about notifications, and what is asked of it: an
 * account's history and choices, what the administrator sends and sets for
 * every account, and what deserves an administrator's look. Kept apart from
 * the rest of the interface's questions, as the rest of this folder is.
 */

import { get, post, put, remove } from "../api";
import type { Picture } from "../api";

/** The colour a notification wears. */
export type Level = "ok" | "attention" | "trouble" | "news";

/** The kinds an account chooses about. */
export type ChoosableKind = "new_content" | "message" | "deletion";

export type Kind = ChoosableKind | "maintenance";

/** One series some episodes arrived in. */
export interface SeriesArrived {
  id: string;
  title: string;
  episodes: number;
}

/** What a notification says, by kind; translated here, never on the server,
 *  except a message, written once by the administrator for everybody. */
export type Said =
  | { kind: "message"; title: string; text: string }
  | { kind: "maintenance"; title: string; text: string }
  | {
      kind: "new_content";
      library: string;
      library_name: string;
      films: number;
      film_titles: string[];
      series: SeriesArrived[];
    }
  | { kind: "deletion"; titles: string[]; works: number; checked_gone: number | null };

/** One notification kept for this account. */
export interface Note {
  id: string;
  kind: Kind;
  level: Level;
  data: Said | null;
  /** The work it opens, absent once the work is gone. */
  work_id: string | null;
  poster: Picture[];
  priority: boolean;
  mandatory: boolean;
  sticky: boolean;
  /** How long it stays on the screen; absent, its level decides. */
  shown_for_ms: number | null;
  due_at: string | null;
  created_at: string;
  read: boolean;
}

export interface NotesPage {
  notifications: Note[];
  /** Unread over the whole history. */
  unread: number;
  more: boolean;
}

export interface Channels {
  bell: boolean;
  screen: boolean;
}

export interface KindChoice extends Channels {
  kind: ChoosableKind;
  /** Whether this account chose, rather than following the default. */
  chosen: boolean;
}

/** Quiet hours, in minutes after midnight on this device's clock. */
export interface NoteSettings {
  quiet_from: number | null;
  quiet_until: number | null;
  priority_while_playing: boolean;
  priority_while_quiet: boolean;
}

export interface LibraryHeard {
  id: string;
  name: string;
  announced: boolean;
}

export interface NoteChoices {
  kinds: KindChoice[];
  settings: NoteSettings;
  libraries: LibraryHeard[];
}

export interface DefaultChoice extends Channels {
  kind: ChoosableKind;
}

export interface LibraryAnnouncing {
  id: string;
  name: string;
  announces: boolean;
}

export interface ForEveryAccount {
  defaults: DefaultChoice[];
  libraries: LibraryAnnouncing[];
}

export type Audience =
  | { to: "administrators" }
  | { to: "everyone" }
  | { to: "accounts"; accounts: string[] };

/** A message as the administrator writes it. */
export interface Written {
  level: Level;
  title: string;
  text: string;
  audience: Audience;
  shown_for_seconds: number | null;
  sticky: boolean;
  priority: boolean;
  mandatory: boolean;
  /** For a maintenance: when it happens. */
  due_at: string | null;
}

/** One thing deserving a look. `point` says which; the rest depends on it,
 *  a worry of the summary carrying its own `kind` and what it names. */
export interface AttentionPoint {
  point: "worry" | "refused_sign_ins" | "failed_tasks" | "falling_behind" | "unidentified";
  state: "attention" | "trouble";
  /** Whether marking it as seen quiets it. */
  may_be_seen: boolean;
  count?: number;
  kind?: string;
  label?: string;
  mount?: string;
  used?: number;
}

export const notesApi = {
  /* The newest page of the history, or the one older than a notification
     already shown. */
  page: (before?: string, signal?: AbortSignal) =>
    get<NotesPage>(`/api/v1/notifications${before ? `?before=${before}` : ""}`, signal),
  /* Every one when none is named. */
  markRead: (ids?: string[]) => post<null>("/api/v1/notifications/read", { ids: ids ?? null }),
  markUnread: (ids: string[]) => post<null>("/api/v1/notifications/unread", { ids }),
  remove: (id: string) => remove<null>(`/api/v1/notifications/${id}`),
  choices: (signal?: AbortSignal) => get<NoteChoices>("/api/v1/notifications/choices", signal),
  choose: (kind: ChoosableKind, channels: Channels) =>
    put<null>(`/api/v1/notifications/choices/${kind}`, channels),
  settle: (settings: NoteSettings) => put<null>("/api/v1/notifications/settings", settings),
  hearAbout: (library: string, announced: boolean) =>
    put<null>(`/api/v1/notifications/libraries/${library}`, { announced }),
  forEveryAccount: (signal?: AbortSignal) =>
    get<ForEveryAccount>("/api/v1/system/notifications", signal),
  setDefault: (kind: ChoosableKind, channels: Channels) =>
    put<null>(`/api/v1/system/notifications/defaults/${kind}`, channels),
  setAnnounces: (library: string, announces: boolean) =>
    put<null>(`/api/v1/system/notifications/libraries/${library}`, { announces }),
  write: (written: Written) => post<null>("/api/v1/system/notifications/messages", written),
  /* What deserves a look, for the administrator asking. */
  attention: (signal?: AbortSignal) =>
    get<{ points: AttentionPoint[] }>("/api/v1/system/attention", signal),
  /* Quiets every point that can be, and answers what is left. */
  markAttentionSeen: () => post<{ points: AttentionPoint[] }>("/api/v1/system/attention/seen"),
};
