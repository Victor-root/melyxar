/*
 * Colours written as six hexadecimal digits, taken apart and put together
 * again.
 */

/** A colour's hue in degrees, and its saturation and lightness from 0 to 1. */
export function hslOf(colour: string): { hue: number; saturation: number; lightness: number } {
  const [red, green, blue] = channelsOf(colour);
  const highest = Math.max(red, green, blue);
  const lowest = Math.min(red, green, blue);
  const spread = highest - lowest;
  const lightness = (highest + lowest) / 2;
  const saturation = spread === 0 ? 0 : spread / (1 - Math.abs(2 * lightness - 1));
  const hue =
    spread === 0
      ? 0
      : highest === red
        ? ((green - blue) / spread + 6) % 6
        : highest === green
          ? (blue - red) / spread + 2
          : (red - green) / spread + 4;
  return { hue: hue * 60, saturation, lightness };
}

export function fromHsl(hue: number, saturation: number, lightness: number): string {
  const chroma = (1 - Math.abs(2 * lightness - 1)) * saturation;
  const channel = (offset: number) => {
    const turn = (offset + hue / 30) % 12;
    const value = lightness - chroma / 2 * Math.max(-1, Math.min(turn - 3, 9 - turn, 1));
    return Math.round(value * 255)
      .toString(16)
      .padStart(2, "0");
  };
  return `#${channel(0)}${channel(8)}${channel(4)}`;
}

/** A colour's hue in degrees, and its saturation and value from 0 to 1, which
 *  is how a square of shades is laid out. */
export function hsvOf(colour: string): { hue: number; saturation: number; value: number } {
  const [red, green, blue] = channelsOf(colour);
  const value = Math.max(red, green, blue);
  const spread = value - Math.min(red, green, blue);
  return { hue: hslOf(colour).hue, saturation: value === 0 ? 0 : spread / value, value };
}

export function fromHsv(hue: number, saturation: number, value: number): string {
  const lightness = value * (1 - saturation / 2);
  const shade = lightness === 0 || lightness === 1 ? 0 : (value - lightness) / Math.min(lightness, 1 - lightness);
  return fromHsl(hue, shade, lightness);
}

/** What somebody typed as a colour, as six lowercase digits after a hash, or
 *  nothing when it is not one. The hash and the short form are both allowed. */
export function parseHex(typed: string): string | null {
  const digits = typed.trim().replace(/^#/, "");
  if (!/^([0-9a-f]{3}|[0-9a-f]{6})$/i.test(digits)) {
    return null;
  }
  const full = digits.length === 3 ? [...digits].map((one) => one + one).join("") : digits;
  return `#${full.toLowerCase()}`;
}

/**
 * The most colourful of several colours, or nothing among none: the one
 * whose strongest channel stands furthest from its weakest. Counted that way
 * rather than by saturation, which calls a red so dark it reads as black as
 * saturated as a bright one.
 */
export function mostColourfulOf(colours: string[]): string | null {
  let best: string | null = null;
  let bestSpread = -1;
  for (const colour of colours) {
    const channels = channelsOf(colour);
    const spread = Math.max(...channels) - Math.min(...channels);
    if (spread > bestSpread) {
      best = colour;
      bestSpread = spread;
    }
  }
  return best;
}

function channelsOf(colour: string): [number, number, number] {
  return [1, 3, 5].map((at) => parseInt(colour.slice(at, at + 2), 16) / 255) as [number, number, number];
}
