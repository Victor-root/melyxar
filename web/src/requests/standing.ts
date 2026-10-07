/*
 * Where an answer of a search stands for this account: here already, asked
 * for by it, asked for by others, or free to ask for. Read from what the
 * search answered and from this account's requests as held now, so a
 * request made or taken back shows at once without asking again.
 */

import type { Catalogue, Found, TitleRequest } from "./api";

export type Standing =
  /** Here, whole, in a library this account sees. */
  | { is: "here"; workId: string }
  /** This account asked for it and waits. */
  | { is: "mine"; request: TitleRequest; others: number }
  /** Free to ask for, by others already or not. A series held in part
      names the seasons it holds. */
  | { is: "free"; others: number; heldSeasons: number[] };

/** This account's request for a title, while it waits. */
export function openRequestFor(
  mine: TitleRequest[] | null,
  catalogue: Catalogue,
  tmdbId: string,
): TitleRequest | undefined {
  return mine?.find(
    (one) =>
      one.catalogue === catalogue &&
      one.tmdb_id === tmdbId &&
      (one.state === "pending" || one.state === "accepted"),
  );
}

export function standingOf(found: Found, mine: TitleRequest[] | null): Standing {
  const request = openRequestFor(mine, found.catalogue, found.tmdb_id);
  const others = found.asked_by - (found.mine !== null ? 1 : 0);
  if (request) {
    return { is: "mine", request, others };
  }
  if (found.held && found.catalogue === "films") {
    return { is: "here", workId: found.held.work_id };
  }
  return { is: "free", others, heldSeasons: found.held?.seasons ?? [] };
}
