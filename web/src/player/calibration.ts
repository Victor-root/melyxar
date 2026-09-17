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
 */
export const CALIBRATION_VERSION = 1;

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

async function measureOnce(codec: string, height: number): Promise<Measured> {
  const opened = await api.openCalibrationSession(codec, height);
  const video = hiddenVideo();

  try {
    const { default: Hls } = await import("hls.js");
    if (!Hls.isSupported()) {
      // Nothing about a real streaming path can be measured here, so the
      // codec is left exactly as unmeasured as it always was.
      return { usable: false, droppedShare: 1 };
    }

    const hls = new Hls();
    try {
      hls.attachMedia(video);
      hls.loadSource(opened.playlist_url);
      await new Promise<void>((resolve, reject) => {
        hls.on(Hls.Events.MANIFEST_PARSED, () => resolve());
        hls.on(Hls.Events.ERROR, (_event, data) => {
          if (data.fatal) {
            reject(new Error(data.details));
          }
        });
      });
      await video.play();

      const quality = video.getVideoPlaybackQuality?.();
      const startedAt = { time: video.currentTime, dropped: quality?.droppedVideoFrames ?? 0 };
      await new Promise((resolve) => window.setTimeout(resolve, MEASURE_MS));
      const endQuality = video.getVideoPlaybackQuality?.();
      const advanced = video.currentTime - startedAt.time;
      const dropped = (endQuality?.droppedVideoFrames ?? 0) - startedAt.dropped;
      const shown =
        (endQuality?.totalVideoFrames ?? 0) - (quality?.totalVideoFrames ?? 0);

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
    }
  } catch {
    return { usable: false, droppedShare: 1 };
  } finally {
    video.remove();
    api.closeSession(opened.id);
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

/** Whether a stored calibration was made by the recipe this build still
 *  uses. A profile made an older way is treated as if it did not exist,
 *  rather than trusted for something it never really measured. */
export function isCurrent(entries: { calibration_version: number }[]): boolean {
  return entries.length > 0 && entries.every((entry) => entry.calibration_version === CALIBRATION_VERSION);
}
