/*
 * Whether this device draws the interface lighter, for a phone that cannot
 * keep up with the full look, whatever the look loses: chosen in its
 * settings alone and kept on it, as a television's lighter drawing is.
 *
 * Lighter, nothing is drawn as frosted glass: what the glass stood over is
 * no longer blurred at every frame, and the pieces that were glass are
 * solid, of the colour they were. What opens over the page, the menus, the
 * sheets and the windows, is drawn without its shadow. Measured on a phone
 * of 2016: a menu opened with twice the frames, and a film's page scrolled at
 * 12 ms a frame rather than 30. Every piece is still there and does the
 * same.
 *
 * Said to the stylesheet by a mark on the root (styles/lighter.css), put
 * there before the first drawing.
 */

import { deviceSwitch } from "./device-switch";

const MARK = "lighter";

function mark(on: boolean) {
  if (on) {
    document.documentElement.dataset[MARK] = "";
  } else {
    delete document.documentElement.dataset[MARK];
  }
}

const lighter = deviceSwitch("melyxar.lighter", mark);

/** Whether this device draws lighter, and the switch for it. */
export const useLighter = lighter.useSwitch;

/** Puts the mark for what this device chose, before anything is drawn. */
export function markLighter() {
  mark(lighter.isOn());
}
