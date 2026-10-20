/*
 * A camera relayed by the server, played in a `<video>` by Media Source
 * Extensions: one long answer of fragmented MP4, its pieces handed to the
 * browser as they come and the picture kept at now (see `live-edge.ts`).
 */

import { codecsOf, descriptionEnd } from "./fmp4";
import { BEHIND_AT_MOST, JUMPED_FROM, liveRate } from "./live-edge";

/** How often the picture is looked at, in milliseconds. */
const LOOKED_EVERY_MS = 250;

/** How much already shown is kept, in seconds, before it is let go. */
const KEPT_BEHIND = 10;

/** What became of a camera: this browser cannot play its picture, or it was
 *  lost on the way. */
export type Trouble = "unplayable" | "lost";

/** Plays the camera relayed at `url` in `video`, until the returned function
 *  is called. */
export function playRelayed(
  video: HTMLVideoElement,
  url: string,
  onTrouble: (trouble: Trouble) => void,
): () => void {
  const source = new MediaSource();
  const stopped = new AbortController();
  const shown = URL.createObjectURL(source);
  video.src = shown;

  let buffer: SourceBuffer | null = null;
  const waiting: Uint8Array<ArrayBuffer>[] = [];

  const fail = (trouble: Trouble) => {
    if (!stopped.signal.aborted) {
      stopped.abort();
      onTrouble(trouble);
    }
  };

  /** Hands the browser the next piece, or lets go of what is long shown. */
  const feed = () => {
    if (!buffer || buffer.updating) {
      return;
    }
    const held = buffer.buffered;
    if (
      held.length > 0 &&
      video.currentTime - held.start(0) > 2 * KEPT_BEHIND
    ) {
      buffer.remove(held.start(0), video.currentTime - KEPT_BEHIND);
      return;
    }
    const next = waiting.shift();
    if (next) {
      try {
        buffer.appendBuffer(next);
      } catch {
        fail("lost");
      }
    }
  };

  const read = async () => {
    const answer = await fetch(url, {
      signal: stopped.signal,
      credentials: "same-origin",
    });
    if (!answer.ok || !answer.body) {
      fail("lost");
      return;
    }
    const reader = answer.body.getReader();
    /* What has come before the description of the stream is whole. */
    let start: Uint8Array<ArrayBuffer> = new Uint8Array(0);
    for (;;) {
      const { done, value } = await reader.read();
      if (done) {
        fail("lost");
        return;
      }
      if (buffer) {
        waiting.push(value);
      } else {
        const longer = new Uint8Array(start.length + value.length);
        longer.set(start);
        longer.set(value, start.length);
        start = longer;
        const end = descriptionEnd(start);
        if (end === null) {
          continue;
        }
        const codecs = codecsOf(start.subarray(0, end));
        const type = `video/mp4; codecs="${codecs.join(",")}"`;
        if (codecs.length === 0 || !MediaSource.isTypeSupported(type)) {
          fail("unplayable");
          return;
        }
        buffer = source.addSourceBuffer(type);
        buffer.addEventListener("updateend", feed);
        waiting.push(start);
      }
      feed();
    }
  };

  source.addEventListener(
    "sourceopen",
    () => {
      read().catch(() => fail("lost"));
    },
    { once: true },
  );

  /* At now: played faster while a little behind, taken to now when far. */
  const keeping = window.setInterval(() => {
    const held = video.buffered;
    if (held.length === 0) {
      return;
    }
    const end = held.end(held.length - 1);
    const behind = end - video.currentTime;
    if (behind > JUMPED_FROM || video.currentTime < held.start(0)) {
      video.currentTime = Math.max(held.start(0), end - BEHIND_AT_MOST);
    }
    video.playbackRate = liveRate(end - video.currentTime);
    if (video.paused) {
      video.play().catch(() => {
        /* No sound before the viewer has touched the page: the picture
           still plays, muted, until they turn the sound on. */
        video.muted = true;
        video.play().catch(() => {});
      });
    }
  }, LOOKED_EVERY_MS);

  return () => {
    window.clearInterval(keeping);
    stopped.abort();
    video.removeAttribute("src");
    video.load();
    URL.revokeObjectURL(shown);
  };
}
