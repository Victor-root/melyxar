/*
 * What a viewer can ask the picture to be held to.
 *
 * A fixed ladder rather than two free numbers: nobody wants to type a rate in
 * bits per second, and a list of rungs is what every player that offers this
 * at all offers. Each rung is a size and a rate together, because either on
 * its own produces something nobody asked for: a rate alone on a large picture
 * is a smeared picture, and a size alone leaves the rate wherever it happened
 * to land.
 *
 * The choice travels to the server inside the profile, as a ceiling on the
 * height and on the rate. What that then costs is the server's business: a
 * film already below the ceiling is handed over untouched rather than rebuilt
 * to meet it.
 */

/** One rung of the ladder. */
export interface Quality {
  /** What it is called in an address and in storage, never on screen. */
  key: string;
  /** Tallest picture accepted. Null means the film as it is. */
  height: number | null;
  /** Rate accepted, in bits per second. Null means the film as it is. */
  bitrate: number | null;
}

/**
 * The rungs, best first.
 *
 * The first is not a rung at all: it is the film as it was written, which is
 * what nearly everyone wants nearly always, and it must therefore be the one
 * a player starts on.
 */
export const QUALITIES: Quality[] = [
  { key: "auto", height: null, bitrate: null },
  { key: "1080p-20", height: 1080, bitrate: 20_000_000 },
  { key: "1080p-10", height: 1080, bitrate: 10_000_000 },
  { key: "1080p-6", height: 1080, bitrate: 6_000_000 },
  { key: "720p-4", height: 720, bitrate: 4_000_000 },
  { key: "720p-2", height: 720, bitrate: 2_000_000 },
  { key: "480p-1.5", height: 480, bitrate: 1_500_000 },
  { key: "360p-0.7", height: 360, bitrate: 700_000 },
];

/** The rung a player starts on: the film as it was written. */
export const AS_IT_IS = QUALITIES[0];

/** The rung by that name, or the film as it is when the name means nothing. */
export function qualityCalled(key: string | null): Quality {
  return QUALITIES.find((quality) => quality.key === key) ?? AS_IT_IS;
}

/**
 * What a rung is called on screen.
 *
 * Built from its own numbers rather than translated one by one: "1080p" and a
 * rate are the same words in every language, and a list of eight translated
 * strings is a list of eight strings to get wrong.
 */
export function qualityName(quality: Quality, asItIs: string): string {
  if (quality.height === null || quality.bitrate === null) {
    return asItIs;
  }
  const rate = quality.bitrate / 1_000_000;
  const written = Number.isInteger(rate) ? `${rate}` : rate.toFixed(1);
  return `${quality.height}p · ${written} Mb/s`;
}

/** Where the choice is kept, so a viewer sets it once rather than per film. */
const REMEMBERED = "melyxar.quality";

export function storedQuality(): Quality {
  try {
    return qualityCalled(window.localStorage.getItem(REMEMBERED));
  } catch {
    // A browser refusing storage is a browser that watches films perfectly
    // well, so it gets the film as it is and nothing is said about it.
    return AS_IT_IS;
  }
}

export function rememberQuality(quality: Quality) {
  try {
    window.localStorage.setItem(REMEMBERED, quality.key);
  } catch {
    // The choice still holds for this sitting, which is what is being watched.
  }
}
