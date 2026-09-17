/*
 * What this browser can open, asked of the browser itself.
 *
 * Every browser answers differently, and the answers change with the machine:
 * the same Chromium plays more on one computer than on another. Guessing from
 * the name of the browser is how a film ends up rebuilt for nothing, or handed
 * over untouched to a player that shows a black screen. So each combination is
 * tried out here, once, and the answer travels with the request.
 */

import type { Quality } from "./quality";
import { isCurrent, storedCalibration } from "./calibration";

/** One thing the server will ask about. */
interface Probe {
  /** What to hand the browser. */
  type: string;
  /** What the server calls it. */
  name: string;
}

const CONTAINERS: Probe[] = [
  { type: 'video/mp4; codecs="avc1.640028"', name: "mp4" },
  { type: 'video/webm; codecs="vp9"', name: "webm" },
  { type: 'video/quicktime; codecs="avc1.640028"', name: "mov" },
  { type: 'video/x-matroska; codecs="avc1.640028"', name: "matroska" },
];

const VIDEO: Probe[] = [
  { type: 'video/mp4; codecs="avc1.640028"', name: "h264" },
  { type: 'video/mp4; codecs="hvc1.1.6.L93.B0"', name: "hevc" },
  { type: 'video/webm; codecs="vp9"', name: "vp9" },
  { type: 'video/mp4; codecs="av01.0.08M.08"', name: "av1" },
];

const AUDIO: Probe[] = [
  { type: 'audio/mp4; codecs="mp4a.40.2"', name: "aac" },
  { type: "audio/mpeg", name: "mp3" },
  { type: 'audio/webm; codecs="opus"', name: "opus" },
  { type: 'audio/webm; codecs="vorbis"', name: "vorbis" },
  { type: 'audio/mp4; codecs="flac"', name: "flac" },
  { type: 'audio/mp4; codecs="ec-3"', name: "eac3" },
  { type: 'audio/mp4; codecs="ac-3"', name: "ac3" },
];

/**
 * A codec this browser takes in a stream fed to it piece by piece, and how far
 * it takes it.
 *
 * The height is the whole point. "Do you decode AV1" and "do you decode this
 * film, at this size, as fast as it plays" are different questions, and only
 * the second one decides whether anybody sees a picture.
 */
export interface RebuiltCapability {
  codec: string;
  /** Tallest picture decoded smoothly, and null when nothing was measured. */
  max_height: number | null;
  /** Whether the browser said this decode was power efficient. Null when it
   *  never answered the question, which counts against nothing: most
   *  browsers do not answer it at all. */
  power_efficient: boolean | null;
}

export interface ClientProfile {
  containers: string[];
  video: { codec: string }[];
  /** Codecs this browser takes in a stream fed to it piece by piece. */
  rebuilt_video: RebuiltCapability[];
  audio_codecs: string[];
  max_audio_channels: number | null;
  subtitle_formats: string[];
  supports_hdr: boolean;
  max_height: number | null;
  max_bitrate: number | null;
  can_switch_tracks_in_container: boolean;
}

/**
 * The sizes each rebuilt codec is tried at, tallest first.
 *
 * The rungs a film really comes in. Asking about every height in between would
 * answer a question nobody plays at.
 */
const HEIGHTS = [2160, 1440, 1080, 720, 480];

/** What a picture of that height weighs, roughly, so the question is a real one. */
function weight(height: number): number {
  return height >= 2160 ? 25_000_000 : height >= 1080 ? 8_000_000 : 3_000_000;
}

/**
 * What a browser answered about a codec fed to it piece by piece.
 *
 * Two different nothings, and telling them apart is the whole point. A browser
 * that could not be asked leaves the server as free as it was before, while a
 * browser that was asked and said "not smoothly, at any size" has answered,
 * and the codec is not offered at all. Read the same way, the second would be
 * taken for "no limit", which is the exact mistake this is here to prevent.
 */
interface Answered {
  /** Whether the browser could be asked the question at all. */
  asked: boolean;
  /** Tallest picture it decodes smoothly, when it named one. */
  tallest: number | null;
  /** Whether that decode was power efficient, at the height above.
   *
   * "Smooth" and "on a real decoder" are different questions: a browser
   * without hardware support for a codec can still call it smooth by
   * falling back to software on a machine fast enough to keep up in this
   * synthetic measurement, and still drop pictures once a real film asks
   * more of it. Null when the browser never said. */
  efficient: boolean | null;
}

/**
 * How tall this browser decodes a codec smoothly, fed to it piece by piece.
 *
 * Asked with the size, the rate and the cadence, because that is the question
 * that decides whether a picture appears. Asked without them, a browser answers
 * for the codec in the abstract and says yes to a film it will buffer whole
 * without ever showing a frame of: measured on a 4K film rebuilt into AV1,
 * which the same browser played perfectly well at half that size.
 */
async function tallestSmoothly(type: string): Promise<Answered> {
  const capabilities = navigator.mediaCapabilities;
  if (!capabilities?.decodingInfo) {
    return { asked: false, tallest: null, efficient: null };
  }
  for (const height of HEIGHTS) {
    try {
      const answer = await capabilities.decodingInfo({
        type: "media-source",
        video: {
          contentType: type,
          width: Math.round((height * 16) / 9),
          height,
          bitrate: weight(height),
          framerate: 24,
        },
      });
      // Smooth as well as supported: a browser that decodes a film at two
      // frames a second supports it and shows nobody anything.
      if (answer.supported && answer.smooth) {
        return { asked: true, tallest: height, efficient: answer.powerEfficient };
      }
    } catch {
      // A browser that refuses the question is a browser that cannot be asked.
      return { asked: false, tallest: null, efficient: null };
    }
  }
  return { asked: true, tallest: null, efficient: null };
}

/**
 * Asks the browser what it can play, and says it the way the server reads.
 *
 * Two questions, not one. A film the server hands over whole is opened by the
 * video element itself, and a film the server rebuilds is fed to it in pieces
 * through another part of the browser entirely. The two do not always answer
 * the same, so both are asked, and the server is told which answer is which.
 *
 * The second one is asked once and kept: it is about the machine, and the
 * machine does not change between two films.
 */
let measured: Promise<RebuiltCapability[]> | null = null;

function whatItTakesInPieces(): Promise<RebuiltCapability[]> {
  measured ??= Promise.all(
    VIDEO.map(async (entry) => ({
      name: entry.name,
      takes: takesInPieces(entry.type),
      answered: await tallestSmoothly(entry.type),
    })),
  ).then((answers) =>
    answers
      // A codec the browser was asked about and never called smooth is one it
      // cannot show, whatever it says about taking it. Offering it would be
      // the very thing being measured against.
      .filter((answer) => answer.takes && !(answer.answered.asked && answer.answered.tallest === null))
      .map((answer) => ({
        codec: answer.name,
        max_height: answer.answered.tallest,
        power_efficient: answer.answered.efficient,
      })),
  );
  return measured;
}

/** Whether this browser takes a codec fed to it piece by piece at all. */
function takesInPieces(type: string): boolean {
  // A browser too old to have this part at all takes nothing fed in pieces,
  // and the server then produces the codec no client has ever refused.
  return typeof MediaSource !== "undefined" && MediaSource.isTypeSupported(type);
}

/**
 * What a real calibration measured on this exact device, when there is one
 * to trust.
 *
 * Asked once and kept, for the same reason as the prediction above: a
 * calibration does not change between two films either. Null for a device
 * that has never been calibrated, or whose calibration was made by a recipe
 * this build no longer uses, which is treated the same as never having run
 * one at all rather than trusted for something it did not really measure.
 */
let calibrated: Promise<RebuiltCapability[] | null> | null = null;

function whatWasReallyMeasured(): Promise<RebuiltCapability[] | null> {
  calibrated ??= storedCalibration()
    .then((entries) =>
      isCurrent(entries)
        ? entries
            .filter((entry) => entry.usable)
            .map((entry) => ({
              codec: entry.codec,
              max_height: entry.tested_height,
              power_efficient: true,
            }))
        : null,
    )
    .catch(() => null);
  return calibrated;
}

/**
 * Forgets what was cached about this browser's decode capability.
 *
 * Called once a calibration just finished, so the very next question about
 * this device answers from it instead of from before it existed.
 */
export function forgetMeasuredCapabilities(): void {
  measured = null;
  calibrated = null;
}

export async function clientProfile(asked?: Quality): Promise<ClientProfile> {
  const probe = document.createElement("video");
  // "probably" and "maybe" are the two answers that mean yes; only an empty
  // string is a no, and a browser says "maybe" when it will not commit.
  const plays = (type: string) => probe.canPlayType(type) !== "";

  // What was really measured on this device beats what the browser predicts
  // about itself, whenever there is a measurement current enough to trust.
  const rebuilt = (await whatWasReallyMeasured()) ?? (await whatItTakesInPieces());

  return {
    containers: CONTAINERS.filter((entry) => plays(entry.type)).map((entry) => entry.name),
    video: VIDEO.filter((entry) => plays(entry.type)).map((entry) => ({ codec: entry.name })),
    rebuilt_video: rebuilt,
    audio_codecs: AUDIO.filter((entry) => plays(entry.type)).map((entry) => entry.name),
    // A browser mixes down to what the machine has; it never says how many
    // channels that is. Two is what is safe to assume, and asking for more
    // is how a film comes out with no voices on a stereo setup.
    max_audio_channels: 2,
    subtitle_formats: ["webvtt"],
    // No browser shows wide gamut colour correctly on this platform today, so
    // saying otherwise would hand over a film that looks washed out and grey.
    supports_hdr: false,
    // Only what the viewer asked for. Nothing is assumed from a screen size
    // or a connection: a viewer who wants the film as it is gets the film as
    // it is, and one who asked for less says so.
    max_height: asked?.height ?? null,
    max_bitrate: asked?.bitrate ?? null,
    // A browser cannot switch to another track inside a file it is playing
    // directly: choosing one means the server has to rebuild the stream.
    can_switch_tracks_in_container: false,
  };
}
