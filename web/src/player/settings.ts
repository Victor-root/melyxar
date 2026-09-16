/*
 * What a viewer has decided about the player itself.
 *
 * How it behaves and how big parts of it are drawn, as against how the film is
 * played, which is the engine's, and how the words look, which is next door.
 *
 * Kept on this machine rather than on the server, like the subtitle
 * appearance: it is a decision about this screen. A television in a lounge and
 * a laptop on a train want different answers from the same person, and a
 * setting that follows an account across both is a setting that is wrong on
 * one of them.
 *
 * Every value is read back defensively. What is stored here outlives the build
 * that wrote it, and a player that will not open because a number came back a
 * word is worse than a player that opens on the usual answer.
 */

import { safeRead, safeWrite } from "../i18n";

/**
 * How long the controls stay up once a hand stops moving, in thousandths.
 *
 * Long enough to reach across the screen for a button that is already showing,
 * short enough that a film is not watched through a strip of controls.
 */
export const FADES_AFTER_MS = 2_600;

/** What the preview above the bar is drawn at, as a share of its own size. */
export const PREVIEW_SCALES = [0.75, 1, 1.25, 1.5, 2] as const;

export type PreviewScale = (typeof PREVIEW_SCALES)[number];

export interface PlayerSettings {
  /**
   * Whether the controls stay up instead of fading out.
   *
   * Off for everyone who does not ask: a film is watched, and the controls are
   * in front of it. Somebody who reaches for the bar constantly, or who is
   * driving the player from across a room, wants the other answer.
   */
  keepTheControlsUp: boolean;
  /**
   * How big the little picture above the bar is drawn.
   *
   * A share rather than a size in pixels: the sheets are made at one size and
   * the same share is right whatever that size turns out to be, so this stays
   * true if the sheets are ever made larger.
   */
  previewScale: PreviewScale;
}

export const DEFAULT_SETTINGS: PlayerSettings = {
  keepTheControlsUp: false,
  previewScale: 1,
};

const STORED = "melyxar.player";

export function storedSettings(): PlayerSettings {
  const raw = safeRead(STORED);
  if (!raw) {
    return DEFAULT_SETTINGS;
  }
  let held: unknown;
  try {
    held = JSON.parse(raw);
  } catch {
    return DEFAULT_SETTINGS;
  }
  if (typeof held !== "object" || held === null) {
    return DEFAULT_SETTINGS;
  }
  const values = held as Record<string, unknown>;
  return {
    keepTheControlsUp:
      typeof values.keepTheControlsUp === "boolean"
        ? values.keepTheControlsUp
        : DEFAULT_SETTINGS.keepTheControlsUp,
    previewScale: (PREVIEW_SCALES as readonly number[]).includes(values.previewScale as number)
      ? (values.previewScale as PreviewScale)
      : DEFAULT_SETTINGS.previewScale,
  };
}

export function rememberSettings(settings: PlayerSettings): void {
  safeWrite(STORED, JSON.stringify(settings));
}
