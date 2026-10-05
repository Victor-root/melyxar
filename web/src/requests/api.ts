/*
 * What the server says about title requests, and what is asked of it:
 * whether this account may ask, looking a title up, its own requests, and
 * for the administrator the switch, who may ask and the decisions. Kept
 * apart from the rest of the interface's questions, as the rest of this
 * folder is.
 */

import { get, post, put, remove } from "../api";

export type Catalogue = "films" | "series";

export type RequestState = "pending" | "accepted" | "refused" | "added";

export type Decision = "accepted" | "refused" | "added";

export interface RequestAccess {
  enabled: boolean;
  may_ask: boolean;
}

/** One answer of a search, and where it stands. */
export interface Found {
  catalogue: Catalogue;
  tmdb_id: string;
  title: string;
  original_title: string | null;
  year: number | null;
  overview: string | null;
  poster: string | null;
  /** Here already: the work to open when this account may, and for a series
      the seasons it holds. */
  held: { work_id: string | null; seasons: number[] } | null;
  /** How many accounts asked for it and wait. */
  asked_by: number;
  /** This account's own request for it, while it waits. */
  mine: string | null;
}

/** A row of titles to suggest: from a genre this account watches, or the
    most popular of all. */
export interface Shelf {
  /** Nothing for what is popular whatever it is. */
  genre: string | null;
  /** That genre at the provider, which seeing more asks the rest of. */
  genre_id: string | null;
  catalogue: Catalogue;
  items: Found[];
}

/** One season of a series, as it may be asked for. */
export interface SeasonChoice {
  number: number;
  episodes: number;
  held: boolean;
}

/** One person of the cast, as the provider names them. */
export interface TitlePerson {
  name: string;
  character: string | null;
  photo: string | null;
}

/** A title as the provider describes it, whole, with where it stands here. */
export interface TitlePage {
  /** The same answer a search gives, which asking for it starts from. */
  found: Found;
  tagline: string | null;
  imdb_id: string | null;
  release_date: string | null;
  end_date: string | null;
  runtime_minutes: number | null;
  rating: number | null;
  age_rating: string | null;
  genres: string[];
  studios: string[];
  collection: string | null;
  big_poster: string | null;
  backdrop: string | null;
  /** Where each trailer can be watched, the official ones first. */
  trailers: string[];
  cast: TitlePerson[];
  crew: { name: string; role: string }[];
  seasons: SeasonChoice[];
}

/** One account's request, as kept. */
export interface TitleRequest {
  id: string;
  user_name: string;
  catalogue: Catalogue;
  tmdb_id: string;
  title: string;
  year: number | null;
  poster: string | null;
  overview: string | null;
  /** None for a film or a whole series. */
  seasons: number[];
  note: string;
  state: RequestState;
  answer: string;
  work_id: string | null;
  created_at: string;
  decided_at: string | null;
}

export interface Asking {
  catalogue: Catalogue;
  tmdb_id: string;
  seasons: number[];
  note: string;
  language: string;
}

export interface Asker {
  id: string;
  name: string;
  administrator: boolean;
  may_ask: boolean;
}

/** One title asked for, and every open request for it, oldest first. */
export interface Waiting {
  catalogue: Catalogue;
  tmdb_id: string;
  accepted: boolean;
  requests: TitleRequest[];
}

export interface RequestsAdministration {
  enabled: boolean;
  askers: Asker[];
  waiting: Waiting[];
}

export const requestsApi = {
  access: (signal?: AbortSignal) => get<RequestAccess>("/api/v1/requests/access", signal),
  search: (query: string, language: string, signal?: AbortSignal) =>
    get<Found[]>(
      `/api/v1/requests/search?query=${encodeURIComponent(query)}&language=${language}`,
      signal,
    ),
  suggestions: (language: string, signal?: AbortSignal) =>
    get<Shelf[]>(`/api/v1/requests/suggestions?language=${language}`, signal),
  popular: (catalogue: Catalogue, genre: string | null, page: number, language: string, signal?: AbortSignal) =>
    get<Found[]>(
      `/api/v1/requests/popular?catalogue=${catalogue}&page=${page}&language=${language}${
        genre ? `&genre=${encodeURIComponent(genre)}` : ""
      }`,
      signal,
    ),
  seasons: (tmdbId: string, language: string, signal?: AbortSignal) =>
    get<SeasonChoice[]>(
      `/api/v1/requests/series/${encodeURIComponent(tmdbId)}/seasons?language=${language}`,
      signal,
    ),
  title: (catalogue: Catalogue, tmdbId: string, language: string, signal?: AbortSignal) =>
    get<TitlePage>(
      `/api/v1/requests/title/${catalogue}/${encodeURIComponent(tmdbId)}?language=${language}`,
      signal,
    ),
  mine: (signal?: AbortSignal) => get<TitleRequest[]>("/api/v1/requests", signal),
  ask: (asking: Asking) => post<TitleRequest>("/api/v1/requests", asking),
  withdraw: (id: string) => remove<null>(`/api/v1/requests/${id}`),
  administration: (signal?: AbortSignal) =>
    get<RequestsAdministration>("/api/v1/system/requests", signal),
  switch: (enabled: boolean) => put<null>("/api/v1/system/requests/enabled", { enabled }),
  allow: (account: string, may_ask: boolean) =>
    put<null>(`/api/v1/system/requests/askers/${account}`, { may_ask }),
  decide: (catalogue: Catalogue, tmdb_id: string, decision: Decision, answer: string) =>
    post<null>("/api/v1/system/requests/decisions", { catalogue, tmdb_id, decision, answer }),
};
