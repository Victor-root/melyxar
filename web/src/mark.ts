/*
 * The mark of Melyxar in the accent somebody chose: on the page, and in the
 * tab of the browser.
 *
 * The logo is kept as its relief, a grey picture of its light and shade.
 * Laid over a colour in hard light and cut to the logo's shape, it gives the
 * logo back in that colour, every fold of the ribbon included. The relief was
 * fitted so that the vivid red of the usual accent gives the logo exactly as
 * it was drawn; every other accent is made as vivid before it is used, or the
 * logo comes out duller than the red it was drawn in.
 */

import { safeWrite } from "./i18n";

/** How much more saturated than the accent the logo is drawn. */
const MADE_VIVID = 1.35;

/** How light the colour under the relief is, the lightness it was fitted to. */
const LIGHTNESS = 0.45;

/** The relief the tab is drawn from: the tab is never larger than this. */
const RELIEF = "/melyxar-shade-64.png";

/** Where the last icon drawn for the tab is kept, for the page to put up
 *  before the interface starts (see index.html). */
const STORED_TAB = "melyxar.tab";

/** The colour the logo is drawn in for an accent: its hue, made vivid. */
export function vividOf(accent: string): string {
  const [red, green, blue] = [1, 3, 5].map((at) => parseInt(accent.slice(at, at + 2), 16) / 255);
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
  return fromHsl(hue * 60, Math.min(1, saturation * MADE_VIVID), LIGHTNESS);
}

function fromHsl(hue: number, saturation: number, lightness: number): string {
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

/** The colour the tab was last asked to be drawn in, and the icon of the
 *  server's own logo when it wears one, which wins over any colour. */
let colourAsked: string | null = null;
let logoWorn: string | null = null;

/**
 * Draws the tab's icon in this colour, or puts back the one the page was
 * served with when there is none: the usual accent is the logo as drawn.
 */
export function markTheTab(colour: string | null): Promise<void> {
  colourAsked = colour;
  return drawTheTab();
}

/** Puts the icon of the server's own logo in the tab, or with nothing gives
 *  the tab back Melyxar's in the accent. */
export function tabWearsTheLogo(icon: string | null): Promise<void> {
  logoWorn = icon;
  return drawTheTab();
}

async function drawTheTab(): Promise<void> {
  const links = [...document.querySelectorAll<HTMLLinkElement>('link[rel="icon"]')];
  for (const link of links) {
    link.dataset.served ??= link.getAttribute("href") ?? "";
  }
  if (logoWorn !== null) {
    safeWrite(STORED_TAB, logoWorn);
    for (const link of links) {
      link.href = logoWorn;
    }
    return;
  }
  const colour = colourAsked;
  if (colour === null) {
    safeWrite(STORED_TAB, "");
    for (const link of links) {
      link.href = link.dataset.served ?? link.href;
    }
    return;
  }

  const relief = new Image();
  relief.src = RELIEF;
  await relief.decode();
  // A logo put up, or another colour asked for, while the relief was on its
  // way: that one is drawn instead.
  if (logoWorn !== null || colourAsked !== colour) {
    return;
  }
  const size = relief.naturalWidth;
  const canvas = document.createElement("canvas");
  canvas.width = size;
  canvas.height = size;
  const drawing = canvas.getContext("2d");
  if (!drawing) {
    return;
  }
  drawing.fillStyle = colour;
  drawing.fillRect(0, 0, size, size);
  drawing.globalCompositeOperation = "hard-light";
  drawing.drawImage(relief, 0, 0);
  drawing.globalCompositeOperation = "destination-in";
  drawing.drawImage(relief, 0, 0);
  const drawn = canvas.toDataURL("image/png");
  safeWrite(STORED_TAB, drawn);
  for (const link of links) {
    link.href = drawn;
  }
}
