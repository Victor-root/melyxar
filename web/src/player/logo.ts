/*
 * What stands in the corner of the player, above the film.
 *
 * Every media server puts something there, and it is never the same thing
 * twice: the film's own wordmark when one has been fetched, the server's mark
 * when it has been given one, and the title written out when neither exists.
 *
 * One function rather than a picture chosen where the corner is drawn. A
 * corner that picks its own picture is a corner that has to be opened every
 * time one of the three moves, and that is exactly how a fallback ends up
 * written in two places and wrong in one of them.
 */

import { api, pictureSet } from "../api";
import type { Picture, ServerIdentity } from "../api";
import { useEffect, useState } from "react";

/** What to draw in the corner, and which of the three it turned out to be. */
export interface Mark {
  /** Where the picture is, when there is one to draw. */
  url: string | null;
  /** The same picture at every width it was prepared in, for a screen with
   *  fine pixels. Null for a mark that comes in one size only, which is what
   *  the server's own uploaded mark is. */
  srcSet: string | null;
  /** What to write when there is not, and what to name the picture when there
   *  is: a wordmark nobody can see still has to say what film this is. */
  words: string;
  /** Which of the three answered, so the corner can be drawn differently for
   *  a film's own mark and for the server's. */
  whose: "the_film" | "the_server" | "nobody";
}

/**
 * The mark for one film.
 *
 * `drawnTitle` is the film's own title as a picture, which plenty of films do
 * not have: the provider draws no title for them, and the title written out is
 * then the whole answer rather than a fallback for a slow day.
 */
export function markFor(
  title: string,
  drawnTitle: Picture[],
  branding: ServerIdentity | null,
): Mark {
  const drawn = pictureSet(drawnTitle);
  if (drawn) {
    return {
      url: drawn.src,
      srcSet: drawn.srcSet || null,
      words: title,
      whose: "the_film",
    };
  }
  if (branding?.logo) {
    return {
      url: branding.logo,
      srcSet: null,
      words: branding.server_name,
      whose: "the_server",
    };
  }
  return { url: null, srcSet: null, words: title, whose: "nobody" };
}

/**
 * Fetches what the server calls itself, once per page rather than per film.
 *
 * Held in a variable outside the hook because a viewer watching four films in
 * an evening asks four times otherwise, for an answer that changes when an
 * administrator changes it and not before.
 */
let known: ServerIdentity | null = null;
let asking = false;
const following = new Set<(branding: ServerIdentity) => void>();

/** How long a question that went unanswered waits before it is asked again. */
const ASK_AGAIN_AFTER = 2000;

/** Tells every screen showing the server's name or logo what they are now,
 *  once this page has changed them or the server has said. */
export function serverChanged(server: ServerIdentity): void {
  known = server;
  for (const follower of following) follower(server);
}

/**
 * Asks the server once, and again shortly for as long as a screen still wants
 * the answer when it went unanswered: a connection dropped by whatever carries
 * it, a tunnel or a proxy, would otherwise leave the bar at the top without a
 * name until somebody reloaded the page.
 */
function ask(): void {
  if (asking || known) {
    return;
  }
  asking = true;
  api
    .branding()
    .then((answer) => {
      asking = false;
      serverChanged(answer);
    })
    .catch(() => {
      window.setTimeout(() => {
        asking = false;
        if (following.size > 0) {
          ask();
        }
      }, ASK_AGAIN_AFTER);
    });
}

export function useBranding(): ServerIdentity | null {
  const [branding, setBranding] = useState<ServerIdentity | null>(known);

  useEffect(() => {
    following.add(setBranding);
    // Known already, perhaps only since this screen was drawn: the answer can
    // land between the drawing and this, and a screen that did not take it
    // then would never hear it again.
    if (known) {
      setBranding(known);
    } else {
      ask();
    }
    return () => {
      following.delete(setBranding);
    };
  }, []);

  return branding;
}
