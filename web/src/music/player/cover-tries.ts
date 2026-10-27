/*
 * The cover of what is playing, asked for again when it does not come.
 *
 * A cut in the network leaves the request for the next cover hanging, and a
 * browser keeps the picture it had until the new one arrives: the song that
 * started played under the cover of the one before. A cover that fails, or
 * takes too long, is asked for again a few times, in a picture of its own so
 * nothing of the one before shows meanwhile.
 */

/** How long a cover may take before it is asked for again. */
export const COVER_PATIENCE_MS = 8_000;
/** How long after a cover failed it is asked for again. */
export const COVER_AGAIN_AFTER_MS = 3_000;
/** How many times a cover is asked for in all. */
export const COVER_TRIES = 4;

export interface CoverTry {
  src: string | null;
  /** Asked for this many times before this one. */
  times: number;
  state: "loading" | "loaded" | "failed" | "gave_up";
}

/** Where the cover at `src` stands, given where the last one stood. */
export function coverAt(was: CoverTry, src: string | null): CoverTry {
  return was.src === src ? was : { src, times: 0, state: "loading" };
}

/** How long until it is asked for again, or nothing while it need not be. */
export function waitBeforeAgain(cover: CoverTry): number | null {
  if (cover.src === null) {
    return null;
  }
  switch (cover.state) {
    case "loading":
      return COVER_PATIENCE_MS;
    case "failed":
      return COVER_AGAIN_AFTER_MS;
    default:
      return null;
  }
}

/** Asked for again, or given up on after the last try. */
export function again(cover: CoverTry): CoverTry {
  return cover.times + 1 < COVER_TRIES
    ? { ...cover, times: cover.times + 1, state: "loading" }
    : { ...cover, state: "gave_up" };
}

/** Whether a picture is drawn for it: while it loads or once it has. */
export function drawn(cover: CoverTry): boolean {
  return cover.src !== null && (cover.state === "loading" || cover.state === "loaded");
}
