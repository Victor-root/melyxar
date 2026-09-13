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

export interface ClientProfile {
  containers: string[];
  video: { codec: string }[];
  /** Codecs this browser takes in a stream fed to it piece by piece. */
  rebuilt_video: string[];
  audio_codecs: string[];
  max_audio_channels: number | null;
  subtitle_formats: string[];
  supports_hdr: boolean;
  max_height: number | null;
  max_bitrate: number | null;
  can_switch_tracks_in_container: boolean;
}

/**
 * Asks the browser what it can play, and says it the way the server reads.
 *
 * Two questions, not one. A film the server hands over whole is opened by the
 * video element itself, and a film the server rebuilds is fed to it in pieces
 * through another part of the browser entirely. The two do not always answer
 * the same, so both are asked, and the server is told which answer is which.
 */
export function clientProfile(asked?: Quality): ClientProfile {
  const probe = document.createElement("video");
  // "probably" and "maybe" are the two answers that mean yes; only an empty
  // string is a no, and a browser says "maybe" when it will not commit.
  const plays = (type: string) => probe.canPlayType(type) !== "";

  // A browser too old to have this part at all takes nothing fed in pieces,
  // and the server then produces the codec no client has ever refused.
  const takesInPieces = (type: string) =>
    typeof MediaSource !== "undefined" && MediaSource.isTypeSupported(type);

  return {
    containers: CONTAINERS.filter((entry) => plays(entry.type)).map((entry) => entry.name),
    video: VIDEO.filter((entry) => plays(entry.type)).map((entry) => ({ codec: entry.name })),
    rebuilt_video: VIDEO.filter((entry) => takesInPieces(entry.type)).map((entry) => entry.name),
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
