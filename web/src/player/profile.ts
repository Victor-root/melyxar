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
import { currentOnes, storedCalibration } from "./calibration";
import type { CalibrationEntry } from "../api";

/** One thing the server will ask about. */
interface Probe {
  /** What to hand the browser. */
  type: string;
  /** What the server calls it. */
  name: string;
}

const CONTAINERS: Probe[] = [
  { type: 'video/mp4; codecs="avc1.640028"', name: "mp4" },
  { type: 'video/webm; codecs="vp09.00.10.08"', name: "webm" },
  { type: 'video/quicktime; codecs="avc1.640028"', name: "mov" },
  { type: 'video/x-matroska; codecs="avc1.640028"', name: "matroska" },
];

const VIDEO: Probe[] = [
  { type: 'video/mp4; codecs="avc1.640028"', name: "h264" },
  { type: 'video/mp4; codecs="hvc1.1.6.L93.B0"', name: "hevc" },
  { type: 'video/webm; codecs="vp09.00.10.08"', name: "vp9" },
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

/** The codecs a wide gamut film comes in, each asked at ten bits. */
const WIDE_GAMUT: Probe[] = [
  { type: 'video/mp4; codecs="hvc1.2.4.L153.B0"', name: "hevc" },
  { type: 'video/webm; codecs="vp09.02.10.10"', name: "vp9" },
  { type: 'video/mp4; codecs="av01.0.12M.10"', name: "av1" },
];

/** The curves a wide gamut picture is written on, as the browser and the
 *  server each name them. */
const CURVES: { transfer: TransferFunction; name: Curve; metadata?: HdrMetadataType }[] = [
  // The common flavour carries the brightness of the screen it was mastered
  // on, and a browser that cannot read it has no business saying yes.
  { transfer: "pq", name: "pq", metadata: "smpteSt2086" },
  { transfer: "hlg", name: "hlg" },
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

export type Curve = "pq" | "hlg";

/** A codec whose wide gamut picture this browser shows as it is, on one curve. */
export interface WideGamutCapability {
  codec: string;
  curve: Curve;
}

export interface ClientProfile {
  containers: string[];
  video: { codec: string }[];
  /** Codecs this browser takes in a stream fed to it piece by piece. */
  rebuilt_video: RebuiltCapability[];
  audio_codecs: string[];
  max_audio_channels: number | null;
  subtitle_formats: string[];
  /** The wide gamut pictures shown as they are, empty on a screen of
   *  standard range. */
  wide_gamut: WideGamutCapability[];
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
 * What was really measured on this exact device, codec by codec.
 *
 * Asked once and kept, for the same reason as the prediction above: a
 * calibration does not change between two films either. Empty for a device
 * nobody has measured, and short of a codec whenever that codec was never
 * answered for: a server that cannot produce AV1 at all leaves no row for
 * AV1, and a row made by a recipe this build no longer uses answers for a
 * measurement that no longer exists.
 */
let calibrated: Promise<CalibrationEntry[]> | null = null;

function whatWasReallyMeasured(): Promise<CalibrationEntry[]> {
  calibrated ??= storedCalibration()
    .then(currentOnes)
    .catch(() => []);
  return calibrated;
}

/**
 * What the browser predicts about itself, corrected wherever this device was
 * really measured.
 *
 * Codec by codec, and that is the whole of it. A measurement is the better
 * answer where there is one: it watched this machine play a real film rather
 * than asking the machine to guess about itself. Where there is none, the
 * prediction stands, because the alternative is to offer a codec nothing ever
 * looked at or to withhold one nothing ever faulted.
 *
 * A codec the browser says it cannot take at all is never offered whatever a
 * measurement says: that answer is about this build of this browser, and no
 * measurement overrules it.
 */
function whatToTellTheServer(
  predicted: RebuiltCapability[],
  measurements: CalibrationEntry[],
): RebuiltCapability[] {
  return predicted.flatMap((prediction) => {
    const measurement = measurements.find((entry) =>
      entry.codec.toLowerCase() === prediction.codec.toLowerCase(),
    );
    if (!measurement) {
      return [prediction];
    }
    if (!measurement.usable) {
      return [];
    }
    return [
      {
        codec: prediction.codec,
        max_height: measurement.tested_height,
        power_efficient: true,
      },
    ];
  });
}

/**
 * The wide gamut pictures this browser decodes as they are, whatever the
 * screen.
 *
 * Asked once and kept, like the rest of what is about the machine. Asked for a
 * film opened whole and for one fed in pieces alike, because a picture carried
 * over as it is reaches the player either way, and a yes to one only would
 * leave the other showing it washed out.
 */
let wideGamutDecoded: Promise<WideGamutCapability[]> | null = null;

function wideGamutItDecodes(): Promise<WideGamutCapability[]> {
  wideGamutDecoded ??= Promise.all(
    WIDE_GAMUT.flatMap((entry) =>
      CURVES.map(async (curve) => ({
        capability: { codec: entry.name, curve: curve.name },
        decodes: await decodesAsItIs(entry.type, curve),
      })),
    ),
  ).then((answers) => answers.filter((answer) => answer.decodes).map((answer) => answer.capability));
  return wideGamutDecoded;
}

async function decodesAsItIs(type: string, curve: (typeof CURVES)[number]): Promise<boolean> {
  const capabilities = navigator.mediaCapabilities;
  if (!capabilities?.decodingInfo) {
    return false;
  }
  const video: VideoConfiguration = {
    contentType: type,
    width: 3840,
    height: 2160,
    bitrate: weight(2160),
    framerate: 24,
    colorGamut: "rec2020",
    transferFunction: curve.transfer,
    ...(curve.metadata ? { hdrMetadataType: curve.metadata } : {}),
  };
  try {
    const answers = await Promise.all(
      (["file", "media-source"] as const).map((kind) => capabilities.decodingInfo({ type: kind, video })),
    );
    return answers.every((answer) => answer.supported);
  } catch {
    // A browser that refuses the question is taken for one that shows none:
    // converted, a film is never washed out.
    return false;
  }
}

/**
 * The wide gamut pictures this browser shows as they are, right now.
 *
 * The screen is asked every time, since the answer changes with it: high
 * dynamic range switched off in the system, or the window moved to another
 * screen. A decoder that hands the picture over as it is shows it washed out
 * on a screen that is not in that mode.
 */
async function wideGamutShown(): Promise<WideGamutCapability[]> {
  const screenShowsIt = window.matchMedia?.("(dynamic-range: high)").matches ?? false;
  return screenShowsIt ? wideGamutItDecodes() : [];
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
  // about itself, codec by codec.
  const rebuilt = whatToTellTheServer(
    await whatItTakesInPieces(),
    await whatWasReallyMeasured(),
  );

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
    wide_gamut: await wideGamutShown(),
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
