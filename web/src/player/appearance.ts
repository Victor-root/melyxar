/*
 * How the subtitles look.
 *
 * Not a matter of taste alone: subtitles that are too small to read at three
 * metres, or white on a snowy shot, are subtitles nobody can follow. A viewer
 * who cannot read them has no way to fix that unless it is offered here.
 *
 * Every choice is a short list rather than a free value. A colour wheel and a
 * pixel size look generous and leave someone with pale grey text on grey; a
 * handful of choices that all work is what a viewer actually needs.
 *
 * The values themselves live in the stylesheet, which is where every other
 * colour and size in this interface lives. This module only remembers what was
 * chosen and names the classes that carry it.
 */

import { safeRead, safeWrite } from "../i18n";

export const SIZES = ["small", "normal", "large", "huge"] as const;
export const COLOURS = ["white", "yellow", "cyan"] as const;
/** What keeps the words legible against the picture behind them. */
export const EDGES = ["outline", "shadow", "none"] as const;
export const BACKGROUNDS = ["none", "dim", "solid"] as const;
/** How far above the bottom of the picture the words sit. */
export const HEIGHTS = ["bottom", "raised", "high"] as const;

export type Size = (typeof SIZES)[number];
export type Colour = (typeof COLOURS)[number];
export type Edge = (typeof EDGES)[number];
export type Background = (typeof BACKGROUNDS)[number];
export type Height = (typeof HEIGHTS)[number];

export interface Appearance {
  size: Size;
  colour: Colour;
  edge: Edge;
  background: Background;
  height: Height;
}

/**
 * What a viewer gets before choosing anything.
 *
 * White with an outline is what a cinema does and what every player settles
 * on, because it stays readable over both a bright sky and a night scene.
 */
export const DEFAULT_APPEARANCE: Appearance = {
  size: "normal",
  colour: "white",
  edge: "outline",
  background: "none",
  height: "bottom",
};

const STORED = "melyxar.subtitles";

/** Reads back what was chosen, ignoring anything that is not a choice. */
export function storedAppearance(): Appearance {
  const raw = safeRead(STORED);
  if (!raw) {
    return DEFAULT_APPEARANCE;
  }
  let held: unknown;
  try {
    held = JSON.parse(raw);
  } catch {
    // Someone else's data, or ours from before a change. The default is a
    // better answer than a player that will not open.
    return DEFAULT_APPEARANCE;
  }
  if (typeof held !== "object" || held === null) {
    return DEFAULT_APPEARANCE;
  }

  const values = held as Record<string, unknown>;
  const one = <T extends string>(
    among: readonly T[],
    value: unknown,
    fallback: T,
  ): T => (among.includes(value as T) ? (value as T) : fallback);

  return {
    size: one(SIZES, values.size, DEFAULT_APPEARANCE.size),
    colour: one(COLOURS, values.colour, DEFAULT_APPEARANCE.colour),
    edge: one(EDGES, values.edge, DEFAULT_APPEARANCE.edge),
    background: one(BACKGROUNDS, values.background, DEFAULT_APPEARANCE.background),
    height: one(HEIGHTS, values.height, DEFAULT_APPEARANCE.height),
  };
}

export function rememberAppearance(appearance: Appearance): void {
  safeWrite(STORED, JSON.stringify(appearance));
}

/**
 * The classes that carry the choices, for the element wrapping the picture.
 *
 * One class per choice rather than one for the whole combination: the
 * stylesheet then says what each choice means once, instead of once per
 * combination of all five.
 */
export function appearanceClasses(appearance: Appearance): string {
  return [
    `subtitles-size-${appearance.size}`,
    `subtitles-colour-${appearance.colour}`,
    `subtitles-edge-${appearance.edge}`,
    `subtitles-background-${appearance.background}`,
  ].join(" ");
}

/**
 * Which line the words sit on, counted from the bottom of the picture.
 *
 * How high a cue sits cannot be set from a stylesheet: it belongs to the cue
 * itself, so it is applied to each one as the track is read. Negative numbers
 * count up from the bottom, which is what keeps the words in the same place
 * whatever the size of the picture.
 */
export function lineFor(height: Height): number {
  switch (height) {
    case "raised":
      return -4;
    case "high":
      return -7;
    default:
      return -2;
  }
}
