/*
 * What decodes a film on this side: the graphics card, and whether the
 * browser uses it for the film on screen.
 *
 * A film that stutters on one machine and plays on the next is most of the
 * time a browser decoding it without the card, and the journal is where the
 * maintainer looks. No browser hands the driver's version to a page as such:
 * the card is said the way the browser names it, which in some browsers
 * carries the version inside the name.
 */

import { api, type Reading } from "../api";
import { howItDecodes } from "./profile";

/** What the browser answers when it hides the card behind its own name. */
const HIDDEN_BEHIND = ["WebKit WebGL", "Mozilla"];

let named: string | null | undefined;

/**
 * The graphics card as the browser names it, asked once.
 *
 * Its plain name first: some browsers now give the real one there, and asking
 * them the older way only earns a warning. The older way only when the plain
 * name is the browser's own.
 */
function theCard(): string | null {
  if (named !== undefined) {
    return named;
  }
  named = null;
  const gl = document.createElement("canvas").getContext("webgl");
  if (!gl) {
    return named;
  }
  const plain = gl.getParameter(gl.RENDERER);
  if (typeof plain === "string" && plain && !HIDDEN_BEHIND.includes(plain)) {
    named = plain;
  } else {
    const hidden = gl.getExtension("WEBGL_debug_renderer_info");
    const unmasked = hidden ? gl.getParameter(hidden.UNMASKED_RENDERER_WEBGL) : null;
    named = typeof unmasked === "string" && unmasked ? unmasked : null;
  }
  // The drawing surface was only ever there to be asked, and a browser keeps
  // few of them.
  gl.getExtension("WEBGL_lose_context")?.loseContext();
  return named;
}

/** Tells the journal what decodes this film, once its picture is on screen. */
export function sayHowItDecodes(
  reading: Reading,
  film: {
    codec: string;
    width: number;
    height: number;
    frameRate: number | null;
    bitrate: number | null;
    handedOver: "file" | "media-source";
  },
): void {
  void howItDecodes(film.codec, film, film.handedOver).then((decoding) =>
    api
      .tellTheJournal({
        ...reading,
        saw: "how_it_decodes",
        card: theCard(),
        codec: film.codec,
        across: film.width,
        down: film.height,
        frames_per_second: film.frameRate,
        on_the_card: decoding?.onTheCard ?? null,
        smoothly: decoding?.smoothly ?? null,
      })
      // Nothing waits on a line in a journal.
      .catch(() => {}),
  );
}
