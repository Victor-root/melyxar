/*
 * Measuring what this device really decodes, rather than what it guesses.
 *
 * "Optimiser cet appareil" runs a short, real playback through the ordinary
 * streaming pipeline, once per codec, so the automatic choice can be told the
 * truth about this exact machine instead of a browser's own prediction of
 * itself. The prediction in `profile.ts` still runs on its own and still
 * works with nothing done here: this is what makes it trustworthy, not what
 * it depends on.
 *
 * Which film is played is the server's business and it is a real one: the
 * most demanding the library holds. A film this server generated was tried
 * first and it could not be made hard enough, costing a decoder a hundredth
 * of what a real film costs at the same size and the same rate.
 *
 * What is measured here is one thing at a time and nothing is decided by
 * guessing: the picture is watched playing, the browser's own frame counters
 * say what happened to it, and a codec is only blamed for a drop the clock
 * did not also explain by having stalled.
 *
 * The picture is watched where it can really be seen, and that is not a
 * detail. Measured against an element parked off the screen, a browser that
 * decodes three pictures a second of a film wanting twenty four reports no
 * dropped picture at all and a clock running perfectly on time: nothing it
 * managed to decode was ever thrown away, because nothing was ever on its
 * way to a screen to be too late for. Every failing machine looked flawless
 * that way. So the element belongs to the page that asked for this, it is on
 * screen while it plays, and what it shows is what is counted.
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
 *
 * Bumped to 3 when the picture stopped being watched off the screen, and a
 * measurement stopped resting on dropped pictures alone. Both were measured
 * against a machine that really could not play AV1: it was called perfect
 * both times. Every earlier row was measured that way and none of them mean
 * anything under this one.
 *
 * Bumped to 4 when the film stopped being one this server generates. Made as
 * hard as a generated picture can be made, it still cost a decoder a hundredth
 * of what a real film costs, and a machine that cannot play AV1 passed it
 * cleanly. What is measured against now is the most demanding film the library
 * actually holds.
 */
export const CALIBRATION_VERSION = 4;

/** The heights asked for, tallest first: the same ladder a real film's
 *  rebuild would climb down if the tallest one did not hold up. What is
 *  really produced is the server's answer, since it never asks a film to be
 *  taller than it is. */
const HEIGHTS = [2160, 1080];

/** How long the picture plays before anything is counted.
 *
 * A decoder does not fall behind at once, it falls behind as the backlog
 * builds: on the film this was all chased through, the first four seconds
 * lost three pictures and the ten after them lost a hundred. Counting from
 * the first frame measures the one part of a film every machine survives. */
const WARM_UP_MS = 2_000;

/** How long one measurement watches the picture play. Long enough for a
 *  decoder to settle into its real behaviour, short enough that testing
 *  every codec stays inside the half a minute this was described as taking. */
const MEASURE_MS = 6_000;

/** Above this share of dropped pictures, a codec is not offered by
 *  automatic choice on this device. */
const MAX_ACCEPTABLE_DROPPED_SHARE = 0.02;

/** Below this share of the pictures the film asked for over the measurement,
 *  the decoder never kept up at all.
 *
 * The one a dropped share cannot catch: a decoder too slow to produce
 * pictures throws none of them away. Measured on a machine struggling for
 * real, this reads about an eighth; on one playing properly it reads one. */
const MUST_SHOW_AT_LEAST = 0.9;

/** Below this share of the measurement window, the clock did not really
 *  advance: a stall, not a decode failing to keep up, and a codec is never
 *  blamed for what a stall explains. */
const CLOCK_MUST_ADVANCE_AT_LEAST = 0.5;

/** How long the watching of one picture is given before it is given up on.
 *
 * Only the watching. Opening the session is left to take as long as it
 * takes, because the first calibration a server ever runs is also the one
 * where it makes the reference film, and a film being made is not a codec
 * failing. */
const GIVE_UP_AFTER_MS = 20_000;

export interface CalibrationProgress {
  codec: string;
  height: number;
  codecIndex: number;
  totalCodecs: number;
}

interface Watched {
  /**
   * Whether the server could produce this film in this codec at all.
   *
   * False is not a verdict on this machine and must never be written down as
   * one: a server with no card for a codec, or one whose encoder gave way
   * halfway through, has said nothing whatsoever about what this browser can
   * decode. The browser's own prediction is a better answer than a made-up
   * failure, and that is what a codec left unmeasured falls back to.
   */
  produced: boolean;
  usable: boolean;
  droppedShare: number;
  shownShare: number;
}

interface Measured extends Watched {
  /** The height really produced, which is the server's answer and not the
   *  height that was asked for. Null when nothing was produced at all. */
  height: number | null;
}

/** What the browser being unable to show the film looks like. */
const NOTHING_APPEARED: Watched = {
  produced: true,
  usable: false,
  droppedShare: 1,
  shownShare: 0,
};

/** What the server being unable to produce the film looks like. */
const NOTHING_PRODUCED: Measured = { ...NOTHING_APPEARED, produced: false, height: null };

/**
 * Asks for the film in one codec at one height, watches it, and says what
 * happened, keeping apart the two ways it can come to nothing: a server that
 * could not produce it, which is not this machine's doing, and a picture that
 * arrived and could not be shown, which is. Nothing here is allowed to reject
 * and stop the whole run over one codec.
 */
async function measure(
  video: HTMLVideoElement,
  codec: string,
  height: number,
): Promise<Measured> {
  try {
    const opened = await api.openCalibrationSession(codec, height);
    try {
      const gaveUp = new Promise<Watched>((resolve) => {
        window.setTimeout(() => resolve(NOTHING_APPEARED), GIVE_UP_AFTER_MS);
      });
      const watched = await Promise.race([
        watchIt(video, opened.playlist_url, opened.frame_rate),
        gaveUp,
      ]);
      return { ...watched, height: opened.height };
    } finally {
      api.closeSession(opened.id);
    }
  } catch {
    // The server would not open this session: no card for this codec, a
    // codec it is not configured to produce, or a film it could not read.
    // None of that is a fact about this browser.
    return NOTHING_PRODUCED;
  }
}

/** Plays one already-open session in the element the page is showing, and
 *  says what really happened, counted against the rate the film really runs
 *  at. Left to throw on anything that goes wrong; `measure` decides what
 *  that means. */
async function watchIt(
  video: HTMLVideoElement,
  playlistUrl: string,
  frameRate: number,
): Promise<Watched> {
  const { default: Hls } = await import("hls.js");
  if (!Hls.isSupported()) {
    // Nothing about a real streaming path can be measured here, so the codec
    // is left exactly as unmeasured as it always was.
    return { ...NOTHING_APPEARED, produced: false };
  }

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
  /* A fatal error carrying the server's own refusal is the server failing to
     produce this film, which says nothing about this browser's decoder. */
  let serverGaveWay = false;
  let stillConnecting: ((error: Error) => void) | null = null;
  hls.on(Hls.Events.ERROR, (_event, data) => {
    if (data.fatal) {
      fatal = data.details;
      serverGaveWay = (data.response?.code ?? 0) >= 500;
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

    // Let it settle before anything counts: a decoder falls behind as its
    // backlog builds, and the first seconds of any film are the ones every
    // machine survives.
    await new Promise((resolve) => window.setTimeout(resolve, WARM_UP_MS));

    const began = video.getVideoPlaybackQuality?.();
    const beganAt = video.currentTime;
    await new Promise((resolve) => window.setTimeout(resolve, MEASURE_MS));

    if (fatal) {
      return { ...NOTHING_APPEARED, produced: !serverGaveWay };
    }

    const ended = video.getVideoPlaybackQuality?.();
    // Every picture the browser made of this film over the window, whether
    // anybody saw it or not, and the ones it threw away among them.
    const made = (ended?.totalVideoFrames ?? 0) - (began?.totalVideoFrames ?? 0);
    const dropped = (ended?.droppedVideoFrames ?? 0) - (began?.droppedVideoFrames ?? 0);
    const appeared = Math.max(0, made - dropped);
    const wanted = (MEASURE_MS / 1000) * frameRate;

    // Nothing made at all is a measurement that failed, never a film that
    // played flawlessly, which is exactly how it used to read.
    const droppedShare = made > 0 ? dropped / made : 1;
    const shownShare = Math.min(1, appeared / wanted);
    const advanced = video.currentTime - beganAt;
    const clockAdvancedEnough = advanced >= (MEASURE_MS / 1000) * CLOCK_MUST_ADVANCE_AT_LEAST;

    return {
      produced: true,
      // Three ways of asking the same question, and a codec is only offered
      // when all three answer yes: the film kept time, it appeared as often
      // as it was meant to, and next to none of it was thrown away.
      usable:
        clockAdvancedEnough &&
        shownShare >= MUST_SHOW_AT_LEAST &&
        droppedShare <= MAX_ACCEPTABLE_DROPPED_SHARE,
      droppedShare,
      shownShare,
    };
  } finally {
    hls.destroy();
    video.removeAttribute("src");
    video.load();
  }
}

/**
 * Runs the whole calibration, one codec after another, sequentially.
 *
 * Stops going smaller the moment a codec holds up at a height: the tallest
 * one that plays cleanly is the only one worth remembering, the same way a
 * browser's own prediction only ever names the tallest it measured.
 *
 * The element is the page's own, and it plays where it can be seen: what is
 * measured here is only worth anything if the browser was really made to put
 * this film on a screen.
 */
export async function runCalibration(
  video: HTMLVideoElement,
  onProgress?: (progress: CalibrationProgress) => void,
): Promise<void> {
  const clientId = deviceIdentity();
  const codecs = CODECS.filter((codec) => codec !== AUTOMATIC).map((codec) => codec.key);

  for (const [codecIndex, codec] of codecs.entries()) {
    const alreadyMeasured = new Set<number>();
    for (const asked of HEIGHTS) {
      onProgress?.({ codec, height: asked, codecIndex, totalCodecs: codecs.length });
      const { produced, usable, droppedShare, shownShare, height } = await measure(
        video,
        codec,
        asked,
      );
      if (!produced) {
        // This server cannot make this codec out of this film, so nothing at
        // all was learned about this machine. Writing a failure down here
        // would put the server's own limit on the viewer's device for good.
        break;
      }
      // What the server really produced, which is what was really watched: a
      // film shorter than the height asked for answers for its own height,
      // and asking again for a rung it already answered measures it twice.
      const measuredHeight = height ?? asked;
      if (alreadyMeasured.has(measuredHeight)) {
        break;
      }
      alreadyMeasured.add(measuredHeight);
      await api.recordCalibration({
        client_id: clientId,
        codec,
        calibration_version: CALIBRATION_VERSION,
        usable,
        tested_height: measuredHeight,
        dropped_share: droppedShare,
        shown_share: shownShare,
        found_by: "test",
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

/**
 * Whether a stored calibration is one to believe.
 *
 * Two questions, and a profile has to pass both. It has to have been made by
 * the recipe this build still uses, since one made an older way measured
 * something else. And it has to have found at least one codec that really
 * played: a run where every single one failed did not discover a machine
 * that can play nothing, it failed to measure the machine at all, and the
 * browser's own prediction is a better answer than that. Judged on the
 * version alone, such a run read as "optimized for this device" while having
 * concluded nothing works, which is how two separate faults in this
 * measurement went unnoticed.
 */
export function worthTrusting(
  entries: { calibration_version: number; usable: boolean }[],
): boolean {
  return currentOnes(entries).some((entry) => entry.usable);
}

/**
 * The rows made by the recipe this build still uses.
 *
 * Kept codec by codec rather than all or nothing: a codec this server could
 * not produce at all leaves no row, and a codec whose row was written a
 * different way answers for a measurement that no longer exists. Either way
 * the rest of what was measured is still perfectly good, and what is missing
 * falls back to the browser's own prediction of itself.
 */
export function currentOnes<T extends { calibration_version: number }>(entries: T[]): T[] {
  return entries.filter((entry) => entry.calibration_version === CALIBRATION_VERSION);
}
