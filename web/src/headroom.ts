/*
 * Whether the bar at the top is out, from how the page is being scrolled.
 *
 * Read down, it steps aside; the first move back up brings it back, and it
 * stays for as long as the page is not read down again. What decides is not
 * each tick of the wheel but how far the page went since it last turned
 * round: a hand resting on a touchpad shakes the page by a point or two, and
 * a bar that answered each of those would flicker.
 */

/** How far down the page has to go before the bar steps aside. */
const DOWN_BEFORE_HIDING = 12;
/** How far back up it has to come before the bar returns: a little, since
 *  somebody reaching for the bar should not have to go looking for it. */
const UP_BEFORE_SHOWING = 4;

export interface Headroom {
  shown: boolean;
  /** Where the page last turned round: the highest it was while the bar was
   *  out, the lowest while it was away. */
  turnedAt: number;
}

/** Where the bar is once the page is scrolled to `y`, with the top of the
 *  page, where it is always out, ending at `top`. */
export function headroomAt(before: Headroom, y: number, top: number): Headroom {
  if (y <= top) {
    return { shown: true, turnedAt: y };
  }
  if (before.shown) {
    if (y > before.turnedAt + DOWN_BEFORE_HIDING) {
      return { shown: false, turnedAt: y };
    }
    return { shown: true, turnedAt: Math.min(before.turnedAt, y) };
  }
  if (y < before.turnedAt - UP_BEFORE_SHOWING) {
    return { shown: true, turnedAt: y };
  }
  return { shown: false, turnedAt: Math.max(before.turnedAt, y) };
}
