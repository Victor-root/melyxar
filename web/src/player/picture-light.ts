/*
 * How bright and how contrasted the picture is shown, as this device's viewer
 * asked: a film converted to standard range can come out darker than the
 * screen wants. The device alone touches the picture it shows; the server
 * knows nothing of it.
 *
 * The Android application's own player is told the same numbers and draws
 * them with Media3's effects (`PictureLight.kt`).
 */

import type { PlaybackPlan } from "../api";

/** The steps offered, as a share of the picture as it comes. */
export const LIGHTS = [0.8, 0.9, 1, 1.1, 1.2, 1.3, 1.4, 1.5] as const;

export type Light = (typeof LIGHTS)[number];

/** The filter drawing [brightness] then [contrast] on the picture, or
 *  nothing when neither moves it. */
export function lightFilter(brightness: number, contrast: number): string | undefined {
  const parts = [
    brightness !== 1 ? `brightness(${brightness})` : null,
    contrast !== 1 ? `contrast(${contrast})` : null,
  ].filter((part) => part !== null);
  return parts.length > 0 ? parts.join(" ") : undefined;
}

/** A step as the reader's language writes a share: "120%", "120 %". */
export function lightName(light: number, language: string): string {
  return new Intl.NumberFormat(language, { style: "percent" }).format(light);
}

/** Whether the picture is shown in standard range, the only one these are
 *  offered for: a film shown in HDR is the screen's to light. */
export function lightsThePicture(plan: PlaybackPlan): boolean {
  return !plan.wide_gamut || plan.wide_gamut.converted;
}
