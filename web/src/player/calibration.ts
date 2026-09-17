/*
 * Measuring what this device really decodes, rather than what it guesses.
 *
 * "Optimiser cet appareil" runs a short, real playback of a fixed reference
 * film through the ordinary streaming pipeline, once per codec, so the
 * automatic choice can be told the truth about this exact machine instead of
 * a browser's own prediction of itself. The prediction in `profile.ts` still
 * runs on its own and still works with nothing done here: this is what makes
 * it trustworthy, not what it depends on.
 *
 * What is measured here is one thing at a time and nothing is decided by
 * guessing: the picture is watched playing, the browser's own frame counters
 * say what happened to it, and a codec is only blamed for a drop the clock
 * did not also explain by having stalled.
 */

import { api } from "../api";
import { AUTOMATIC, CODECS } from "./codec";
import { deviceIdentity } from "./deviceIdentity";

/**
 * Which recipe of the measurement produced a stored row.
 *
 * Bumped whenever the reference film, the heights tested or how a verdict is
 * judged changes, so a row made the old way is offered a fresh calibration
 * rather than trusted forever. Kept in step by hand with the same constant on
 * the server, which is the only other place a change here would matter.
 *
 * Bumped to 2 when the reference film gained real grain: a fractal alone let
 * a codec with no real hardware decoder look usable, because it never cost
 * enough real bits to tell the two apart. Every row made under version 1
 * measured that easier film, not a real one, and is stale under this one.
 */
export const CALIBRATION_VERSION = 2;

/** The heights tried, tallest first: the same ladder a real film's rebuild
 *  would climb down if the tallest one did not hold up. */
const HEIGHTS = [2160, 1080];

/** How long one measurement watches the picture play. Long enough for a
 *  decoder to settle into its real behaviour, short enough that testing
 *  every codec stays inside the half a minute this was described as taking. */
const MEASURE_MS = 6_000;

/** Above this share of dropped pictures, a codec is not offered by
 *  automatic choice on this device. */
const MAX_ACCEPTABLE_DROPPED_SHARE = 0.02;

/** Below this share of the measurement window, the clock did not really
 *  advance: a stall, not a decode failing to keep up, and a codec is never
 *  blamed for what a stall explains. */
const CLOCK_MUST_ADVANCE_AT_LEAST = 0.5;

/** How long a single measurement is given before it is given up on. */
const GIVE_UP_AFTER_MS = 20_000;

export interface CalibrationProgress {
  codec: string;
  height: number;
  codecIndex: number;
  totalCodecs: number;
}

interface Measured {
  usable: boolean;
  droppedShare: number;
}

/** A hidden element, playing, never shown and never in anyone's way. */
function hiddenVideo(): HTMLVideoElement {
  const video = document.createElement("video");
  video.muted = true;
  video.playsInline = true;
  video.style.position = "fixed";
  video.style.left = "-9999px";
  video.style.width = "2px";
  video.style.height = "2px";
  document.body.appendChild(video);
  return video;
}

/** Plays one codec at one height for a moment and says what really happened. */
async function measure(codec: string, height: number): Promise<Measured> {
  const gaveUp = new Promise<Measured>((resolve) => {
    window.setTimeout(() => resolve({ usable: false, droppedShare: 1 }), GIVE_UP_AFTER_MS);
  });
  return Promise.race([measureOnce(codec, height), gaveUp]);
}

/**
 * Everything that can go wrong here, from a server that will not open this
 * session at all (no card for this codec, say) to the picture never arriving,
 * means the same thing to a calibration: this codec is not one to offer on
 * this device. Nothing here is allowed to reject and stop the whole run over
 * one codec the server or the browser could not produce.
 */
async function measureOnce(codec: string, height: number): Promise<Measured> {
  try {
    const opened = await api.openCalibrationSession(codec, height);
    try {
      return await watchIt(opened.playlist_url);
    } finally {
      api.closeSession(opened.id);
    }
  } catch {
    return { usable: false, droppedShare: 1 };
  }
}

/** Plays one already-open session and says what really happened. Left to
 *  throw on anything that goes wrong; `measureOnce` decides what that means. */
async function watchIt(playlistUrl: string): Promise<Measured> {
  const { default: Hls } = await import("hls.js");
  if (!Hls.isSupported()) {
    // Nothing about a real streaming path can be measured here, so the codec
    // is left exactly as unmeasured as it always was.
    return { usable: false, droppedShare: 1 };
  }

  const video = hiddenVideo();
  const hls = new Hls();
  // A fatal error can arrive at any point, including well after the
  // manifest was parsed and the picture already started playing: a segment
  // the media tool fails to produce partway through is exactly that, and it
  // says nothing a playback quality counter would catch on its own, since
  // the picture that did arrive before it played back perfectly well. Kept
  // for the rest of this measurement, not just the wait below, so a codec
  // that failed here is never read as one that merely stalled or was never
  // asked to do anything.
  let fatal: string | null = null;
  let stillConnecting: ((error: Error) => void) | null = null;
  hls.on(Hls.Events.ERROR, (_event, data) => {
    if (data.fatal) {
      fatal = data.details;
      stillConnecting?.(new Error(data.details));
    }
  });
  try {
    hls.attachMedia(video);
    hls.loadSource(playlistUrl);
    await new Promise<void>((resolve, reject) => {
      stillConnecting = reject;
      hls.once(Hls.Events.MANIFEST_PARSED, () => resolve());
    });
    stillConnecting = null;
    await video.play();

    const quality = video.getVideoPlaybackQuality?.();
    const startedAt = { time: video.currentTime, dropped: quality?.droppedVideoFrames ?? 0 };
    await new Promise((resolve) => window.setTimeout(resolve, MEASURE_MS));

    if (fatal) {
      return { usable: false, droppedShare: 1 };
    }

    const endQuality = video.getVideoPlaybackQuality?.();
    const advanced = video.currentTime - startedAt.time;
    const dropped = (endQuality?.droppedVideoFrames ?? 0) - startedAt.dropped;
    const shown = (endQuality?.totalVideoFrames ?? 0) - (quality?.totalVideoFrames ?? 0);

    const clockAdvancedEnough = advanced >= (MEASURE_MS / 1000) * CLOCK_MUST_ADVANCE_AT_LEAST;
    const droppedShare = shown + dropped > 0 ? dropped / (shown + dropped) : 0;

    return {
      // A stall is a buffering problem, not a decode one, and a codec is
      // never blamed for what a stall already explains.
      usable: clockAdvancedEnough && droppedShare <= MAX_ACCEPTABLE_DROPPED_SHARE,
      droppedShare,
    };
  } finally {
    hls.destroy();
    video.remove();
  }
}

/**
 * Runs the whole calibration, one codec after another, sequentially.
 *
 * Stops going smaller the moment a codec holds up at a height: the tallest
 * one that plays cleanly is the only one worth remembering, the same way a
 * browser's own prediction only ever names the tallest it measured.
 */
export async function runCalibration(
  onProgress?: (progress: CalibrationProgress) => void,
): Promise<void> {
  const clientId = deviceIdentity();
  const codecs = CODECS.filter((codec) => codec !== AUTOMATIC).map((codec) => codec.key);

  for (const [codecIndex, codec] of codecs.entries()) {
    for (const height of HEIGHTS) {
      onProgress?.({ codec, height, codecIndex, totalCodecs: codecs.length });
      const { usable, droppedShare } = await measure(codec, height);
      await api.recordCalibration({
        client_id: clientId,
        codec,
        calibration_version: CALIBRATION_VERSION,
        usable,
        tested_height: height,
        dropped_share: droppedShare,
      });
      if (usable) {
        break;
      }
    }
  }
}

/** What this device has had measured so far, straight from the server. */
export function storedCalibration() {
  return api.calibrationProfile(deviceIdentity());
}

/** Forgets this device's calibration, all codecs at once, so the next run
 *  starts from nothing rather than refining what is already there. */
export function resetCalibration() {
  return api.forgetCalibration(deviceIdentity());
}

/** Whether a stored calibration was made by the recipe this build still
 *  uses. A profile made an older way is treated as if it did not exist,
 *  rather than trusted for something it never really measured. */
export function isCurrent(entries: { calibration_version: number }[]): boolean {
  return entries.length > 0 && entries.every((entry) => entry.calibration_version === CALIBRATION_VERSION);
}
