/*
 * What a calibration concludes, kept apart from how it is run.
 *
 * Pure answers to three questions: did one clip play cleanly, what is the
 * tallest height a codec held up at, and what does this device then tell the
 * server. Nothing here plays anything, so all of it is tested on its own.
 */

import type { CodecResult, DeviceCalibration, Measurement } from "../api";
import type { RebuiltCapability } from "../player/profile";

/** Above this share of the pictures made, thrown away, a clip did not play
 *  cleanly. Three pictures in a hundred and fifty at the usual rate. */
export const MAX_DROPPED_SHARE = 0.02;

/** Below this share of the pictures the clip asked for, the decoder never
 *  kept up: a decoder too slow to make pictures throws none of them away,
 *  so a dropped share alone never catches it. */
export const MUST_SHOW_AT_LEAST = 0.9;

/** Below this share of the time watched, the clock itself fell behind. The
 *  clip is played from memory, so nothing but the decoder can make it. */
export const CLOCK_MUST_KEEP_AT_LEAST = 0.9;

/** What the browser's own counters said over one watched stretch. */
export interface Counted {
  /** Pictures made over the stretch, whether anybody saw them or not. */
  made: number;
  /** Pictures thrown away among them. */
  dropped: number;
  /** How far the film's clock moved, in seconds. */
  advanced: number;
  /** How long the stretch lasted, in seconds. */
  watched: number;
  frameRate: number;
}

/** Whether one clip played cleanly, and the two shares it was judged on. */
export function judge(height: number, counted: Counted): Measurement {
  const wanted = counted.watched * counted.frameRate;
  const appeared = Math.max(0, counted.made - counted.dropped);
  // Nothing made at all is a decoder that showed nothing, never one that
  // played flawlessly.
  const dropped_share = counted.made > 0 ? counted.dropped / counted.made : 1;
  const shown_share = wanted > 0 ? Math.min(1, appeared / wanted) : 0;
  const keptTime = counted.advanced >= counted.watched * CLOCK_MUST_KEEP_AT_LEAST;
  return {
    height,
    passed: keptTime && shown_share >= MUST_SHOW_AT_LEAST && dropped_share <= MAX_DROPPED_SHARE,
    dropped_share,
    shown_share,
  };
}

/** The tallest height a codec held up at, among measurements made tallest
 *  first, or null when it held up at none. */
export function smoothHeight(measurements: Measurement[]): number | null {
  return measurements.find((measurement) => measurement.passed)?.height ?? null;
}

/** One codec's answer, from what its heights measured. */
export function resultFor(codec: string, measurements: Measurement[]): CodecResult {
  return { codec, smooth_height: smoothHeight(measurements), measurements };
}

/**
 * What this device tells the server it decodes once a picture is rebuilt.
 *
 * Without a whole calibration, exactly what the browser predicts about itself,
 * as it always has: a calibration that never finished changes nothing. With
 * one, only what was measured, each codec with the tallest height it held up
 * at, and never a codec the browser cannot take in pieces at all.
 */
export function whatToTellTheServer(
  predicted: RebuiltCapability[],
  calibration: DeviceCalibration | null,
  takesInPieces: (codec: string) => boolean,
): RebuiltCapability[] {
  if (!calibration) {
    return predicted;
  }
  return calibration.codecs.flatMap((result) =>
    result.smooth_height !== null && takesInPieces(result.codec)
      ? [{ codec: result.codec, max_height: result.smooth_height, power_efficient: true }]
      : [],
  );
}
