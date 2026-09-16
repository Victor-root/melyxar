/*
 * What stands in the corner of the player, above the film.
 *
 * Every media server puts something there, and it is never the same thing
 * twice: the film's own wordmark when one has been fetched, the server's mark
 * when it has been given one, and the title written out when neither exists.
 *
 * One function rather than a picture chosen where the corner is drawn. All
 * three of these are going to move: film logos are not fetched at all yet, and
 * the server's mark is a setting somebody will change. A corner that picks its
 * own picture is a corner that has to be opened every time one of the three
 * does, and that is exactly how a fallback ends up written in two places and
 * wrong in one of them.
 */

import { api } from "../api";
import { useEffect, useState } from "react";

/** What to draw in the corner, and which of the three it turned out to be. */
export interface Mark {
  /** Where the picture is, when there is one to draw. */
  url: string | null;
  /** What to write when there is not, and what to name the picture when there
   *  is: a wordmark nobody can see still has to say what film this is. */
  words: string;
  /** Which of the three answered, so the corner can be drawn differently for
   *  a film's own mark and for the server's. */
  whose: "the_film" | "the_server" | "nobody";
}

/** What the server calls itself and the mark it was given, fetched once. */
interface Branding {
  server_name: string;
  logo_path: string | null;
}

/**
 * The mark for one film.
 *
 * `filmLogo` is what has been fetched for this film, and is null until logos
 * are fetched at all. Written as an argument rather than read from the plan so
 * that the day they arrive, the only change is what is handed in.
 */
export function markFor(title: string, filmLogo: string | null, branding: Branding | null): Mark {
  if (filmLogo) {
    return { url: filmLogo, words: title, whose: "the_film" };
  }
  if (branding?.logo_path) {
    return {
      url: `/api/v1/images/${branding.logo_path}`,
      words: branding.server_name,
      whose: "the_server",
    };
  }
  return { url: null, words: title, whose: "nobody" };
}

/**
 * Fetches what the server calls itself, once per page rather than per film.
 *
 * Held in a variable outside the hook because a viewer watching four films in
 * an evening asks four times otherwise, for an answer that changes when an
 * administrator changes it and not before.
 */
let known: Branding | null = null;
let asking: Promise<Branding | null> | null = null;

export function useBranding(): Branding | null {
  const [branding, setBranding] = useState<Branding | null>(known);

  useEffect(() => {
    if (known) {
      return;
    }
    let gone = false;
    asking =
      asking ??
      api
        .branding()
        .then((answer) => {
          known = answer;
          return answer;
        })
        .catch(() => null);
    void asking.then((answer) => {
      if (!gone && answer) {
        setBranding(answer);
      }
    });
    return () => {
      gone = true;
    };
  }, []);

  return branding;
}
