/*
 * What watching a real film taught this device.
 *
 * The test in `calibration.ts` asks a question in half a minute, against one
 * film, from a standing start. Watching answers the same question against
 * every film this person actually plays, for as long as they play it, and it
 * costs nothing: the pictures are being decoded either way, and the browser
 * counts what it did with them whether anybody reads the count or not.
 *
 * So a codec that really stuttered its way through a real film is written
 * down as one this machine cannot be offered again, and that verdict outranks
 * anything the test concluded. Nothing here ever writes the good news: a film
 * that played cleanly proves only that this film played cleanly, and the next
 * one may be heavier. Only the test, or forgetting this device's calibration,
 * puts a codec back in the running.
 *
 * A stall is never a codec's fault. The clock stopping means the server or
 * the link could not keep the browser fed, and stretches where it stopped are
 * left out of the count rather than charged to the decoder.
 */

import { api } from "../api";
import { CALIBRATION_VERSION } from "./calibration";
import { deviceIdentity } from "../deviceIdentity";
import { forgetMeasuredCapabilities } from "./profile";

/** How often what the browser did is read again. */
const LOOK_EVERY_MS = 1_000;

/**
 * How many pictures have to have gone by before this says anything.
 *
 * Ten seconds of a film, near enough. Long enough that the handful of
 * pictures a jump costs cannot carry a verdict on its own, short enough that
 * a codec this machine cannot play is caught inside the opening scene rather
 * than at the end of the film.
 */
const ENOUGH_PICTURES_TO_JUDGE = 240;

/**
 * Above this share of pictures dropped, this codec is not offered again.
 *
 * Measured on the maintainer's machines: one playing properly drops none at
 * all over a whole film, and one without a hardware decoder for the codec
 * dropped between a fifth and half of every stretch. A twentieth sits far
 * above the one or two a jump costs and far below anything that reads as
 * stuttering.
 */
const TOO_MANY_DROPPED = 0.05;

/**
 * How much of a stretch the clock has to cover for it to count.
 *
 * Below this the film was waiting rather than playing, and what a decoder did
 * while there was nothing to decode says nothing about the decoder.
 */
const CLOCK_MUST_COVER = 0.5;

/** What a film is taken to run at when nothing read its rate. The lenient way
 *  to be wrong, matching what the server assumes for the same reason. */
const FRAME_RATE_WHEN_UNREAD = 24;

/** What the picture being rebuilt is, as the plan describes it. */
export interface BeingRebuilt {
  codec: string;
  /** Pictures a second, when the file said. */
  frameRate: number | null;
}

/** What following one film for this hands back. */
export interface Learning {
  stop: () => void;
}

/**
 * Follows one film and writes down a codec this machine could not keep up
 * with, once it is sure of it.
 */
export function learnFromWatching(
  element: HTMLVideoElement,
  rebuilt: BeingRebuilt,
): Learning {
  const rate = rebuilt.frameRate ?? FRAME_RATE_WHEN_UNREAD;

  /* Only the stretches the film really played over. Everything else, a pause,
     a jump, a stall, is dropped from the count rather than charged to the
     decoder. */
  let made = 0;
  let dropped = 0;
  let appeared = 0;
  let owed = 0;
  let told = false;

  const quality = () => element.getVideoPlaybackQuality?.();
  let lastMade = quality()?.totalVideoFrames ?? 0;
  let lastDropped = quality()?.droppedVideoFrames ?? 0;
  let lastAt = element.currentTime;
  let lastLooked = performance.now();

  const look = () => {
    const now = performance.now();
    const counted = quality();
    const at = element.currentTime;
    const wall = (now - lastLooked) / 1000;
    const advanced = at - lastAt;

    const startAgainFromHere = () => {
      lastMade = counted?.totalVideoFrames ?? 0;
      lastDropped = counted?.droppedVideoFrames ?? 0;
      lastAt = at;
      lastLooked = now;
    };

    const playing = !element.paused && !element.ended && !element.seeking;
    if (!playing || wall <= 0 || advanced < wall * CLOCK_MUST_COVER) {
      startAgainFromHere();
      return;
    }

    made += (counted?.totalVideoFrames ?? 0) - lastMade;
    dropped += (counted?.droppedVideoFrames ?? 0) - lastDropped;
    appeared = Math.max(0, made - dropped);
    owed += advanced * rate;
    startAgainFromHere();

    if (told || made < ENOUGH_PICTURES_TO_JUDGE) {
      return;
    }
    const droppedShare = dropped / made;
    if (droppedShare <= TOO_MANY_DROPPED) {
      return;
    }

    // Said once for this film, and then this stops counting: the answer is
    // in, and going on would only write it again.
    told = true;
    api
      .recordCalibration({
        client_id: deviceIdentity(),
        codec: rebuilt.codec,
        calibration_version: CALIBRATION_VERSION,
        usable: false,
        // What was really decoded, which is the picture the browser ended up
        // with rather than anything the plan asked for.
        tested_height: element.videoHeight || 0,
        dropped_share: droppedShare,
        shown_share: owed > 0 ? Math.min(1, appeared / owed) : 0,
        found_by: "watching",
      })
      .then(() => {
        // The next film has to answer from this, not from what was cached
        // before this film proved it wrong.
        forgetMeasuredCapabilities();
      })
      .catch(() => {
        // A verdict that did not reach the server is one the next film will
        // reach the same conclusion about. Nothing here is worth troubling a
        // viewer over in the middle of a film.
      });
  };

  const ticking = window.setInterval(look, LOOK_EVERY_MS);
  return {
    stop: () => window.clearInterval(ticking),
  };
}
