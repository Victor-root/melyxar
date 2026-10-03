/*
 * Running a calibration: what this device really decodes, measured.
 *
 * The server cuts short clips out of a real film, in every codec it writes
 * and at every height up to the film's own. Each one is downloaded whole
 * before it is played, so nothing but this device's decoding can make it
 * stutter: not the server producing it, not the network. The calibration this
 * replaces watched a film produced live, and a slow server made a capable
 * device look incapable.
 *
 * The picture plays where it can be seen, and that is not a detail: measured
 * against an element off the screen, a browser that decodes three pictures a
 * second reports none dropped, since nothing it decoded was on its way to a
 * screen to be late for. A stretch watched while the page was hidden is
 * thrown away and watched again, never counted.
 *
 * Whole or nothing. The result is handed to the server once, at the end, and
 * only when every codec has its answer. A run that stops anywhere before that
 * leaves nothing behind, and the device stays exactly as if it had never been
 * calibrated.
 */

import { api } from "../api";
import type { CalibrationClip, CalibrationClips, Measurement } from "../api";
import { deviceIdentity } from "../deviceIdentity";
import { judge, resultFor } from "./judge";

/** How long a clip plays before anything is counted: a decoder falls behind
 *  as its backlog builds, not at the first picture. */
const WARM_UP_MS = 2_000;

/** How long a clip is watched, out of the twelve seconds it lasts. */
const WATCH_MS = 9_000;

/** How often the page is checked for still being on screen. */
const LOOK_EVERY_MS = 250;

/** How many tries one clip gets that actually count. A decoder can stumble
 *  once on something the machine was doing at the same moment; a clip that
 *  plays cleanly once is a clip the device plays. */
const TRIES_THAT_COUNT = 2;

/** How many times in a row a clip is watched with the page hidden before the
 *  run is given up: somebody who left the page is not watching it. */
const HIDDEN_TOLERATED = 3;

/** How often the server is asked how its preparation is going. */
const ASK_EVERY_MS = 1_000;

/** The type a browser is asked whether it opens, codec by codec. */
const OPENS: Record<string, string> = {
  h264: 'video/mp4; codecs="avc1.640033"',
  hevc: 'video/mp4; codecs="hvc1.1.6.L153.B0"',
  av1: 'video/mp4; codecs="av01.0.12M.08"',
};

export type CalibrationProgress =
  | { phase: "preparing"; done: number; total: number }
  | { phase: "measuring"; codec: string; height: number; codecIndex: number; totalCodecs: number };

/** Why a run stopped before it finished. */
export class Interrupted extends Error {}

const pause = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms));

/** Waits for the server's clips, asking it to make them first if it has not. */
async function readyClips(onProgress: (progress: CalibrationProgress) => void): Promise<CalibrationClips> {
  let clips = await api.calibrationClips();
  if (clips.state === "not_prepared" || clips.state === "failed") {
    await api.prepareCalibrationClips();
    clips = await api.calibrationClips();
  }
  while (clips.state === "preparing") {
    onProgress({ phase: "preparing", done: clips.done, total: clips.total });
    await pause(ASK_EVERY_MS);
    clips = await api.calibrationClips();
  }
  if (clips.state !== "ready") {
    throw new Interrupted(clips.reason ?? "the server could not make the clips");
  }
  return clips;
}

const onScreen = () => document.visibilityState === "visible";

/** Waits, and says whether the page stayed on screen the whole time. */
async function watchedWhileVisible(ms: number): Promise<boolean> {
  for (let waited = 0; waited < ms; waited += LOOK_EVERY_MS) {
    await pause(LOOK_EVERY_MS);
    if (!onScreen()) {
      return false;
    }
  }
  return true;
}

/** Plays one clip once and judges it, or answers null when the page was not
 *  on screen for all of it and nothing can be concluded. */
async function playOnce(
  video: HTMLVideoElement,
  source: string,
  clip: CalibrationClip,
): Promise<Measurement | null> {
  video.src = source;
  try {
    await video.play();
    if (!(await watchedWhileVisible(WARM_UP_MS))) {
      return null;
    }
    const began = video.getVideoPlaybackQuality();
    const beganAt = video.currentTime;
    const started = performance.now();
    if (!(await watchedWhileVisible(WATCH_MS))) {
      return null;
    }
    const ended = video.getVideoPlaybackQuality();
    return judge(clip.height, {
      made: ended.totalVideoFrames - began.totalVideoFrames,
      dropped: ended.droppedVideoFrames - began.droppedVideoFrames,
      advanced: video.currentTime - beganAt,
      watched: (performance.now() - started) / 1000,
      frameRate: clip.frame_rate,
    });
  } finally {
    video.pause();
    video.removeAttribute("src");
    video.load();
  }
}

/** Measures one clip: passed as soon as one try plays cleanly, failed when
 *  every try that counted failed. */
async function measureClip(video: HTMLVideoElement, clip: CalibrationClip): Promise<Measurement> {
  const source = URL.createObjectURL(await api.calibrationClip(clip.url));
  try {
    let last: Measurement | null = null;
    let counted = 0;
    let hiddenInARow = 0;
    while (counted < TRIES_THAT_COUNT) {
      const measured = await playOnce(video, source, clip);
      if (measured === null) {
        hiddenInARow += 1;
        if (hiddenInARow >= HIDDEN_TOLERATED) {
          throw new Interrupted("the page was not on screen");
        }
        continue;
      }
      hiddenInARow = 0;
      counted += 1;
      last = measured;
      if (measured.passed) {
        return measured;
      }
    }
    return last as Measurement;
  } finally {
    URL.revokeObjectURL(source);
  }
}

/**
 * Runs a whole calibration and keeps it.
 *
 * Every codec the server offers, every height tallest first, stopping at the
 * first height a codec holds up at. A codec this browser does not open at all
 * is answered without playing anything. Anything that goes wrong on the way
 * rejects, and nothing is kept.
 */
export async function runCalibration(
  video: HTMLVideoElement,
  onProgress: (progress: CalibrationProgress) => void,
): Promise<void> {
  const ready = await readyClips(onProgress);
  const codecs = [...new Set(ready.clips.map((clip) => clip.codec))];

  const results = [];
  for (const [codecIndex, codec] of codecs.entries()) {
    const clips = ready.clips
      .filter((clip) => clip.codec === codec)
      .sort((a, b) => b.height - a.height);
    const measurements: Measurement[] = [];
    if (video.canPlayType(OPENS[codec] ?? "") !== "") {
      for (const clip of clips) {
        onProgress({
          phase: "measuring",
          codec,
          height: clip.height,
          codecIndex,
          totalCodecs: codecs.length,
        });
        const measured = await measureClip(video, clip);
        measurements.push(measured);
        if (measured.passed) {
          break;
        }
      }
    }
    results.push(resultFor(codec, measurements));
  }

  await api.recordCalibration(deviceIdentity(), {
    calibration_version: ready.calibration_version,
    codecs: results,
  });
}

/** This device's calibration, or null when it has no whole one. */
export function storedCalibration() {
  return api.deviceCalibration(deviceIdentity());
}

/** Forgets this device's calibration. */
export function resetCalibration() {
  return api.forgetCalibration(deviceIdentity());
}
