/*
 * What the player puts on screen, named, and where each of it sits.
 *
 * The layout is data rather than markup. Every control has a name, every place
 * a control can sit has a name, and the arrangement is a list of names per
 * place. Drawing the player is then walking that list, and nothing about the
 * order or the presence of a control is written into the drawing.
 *
 * That is the whole point. A viewer is going to be able to hide a button they
 * never press and move another one somewhere it suits them, and the difference
 * between that being a setting and that being a rewrite is whether the layout
 * was ever data. Today nothing changes it and everyone gets the same
 * arrangement; the day a settings screen writes one, nothing here has to move.
 *
 * A control the arrangement names but this film has nothing for, a subtitle
 * button on a film carrying no subtitles, simply draws nothing. Leaving it out
 * of the arrangement is a viewer's decision; having nothing to show is the
 * film's, and the two must not be confused.
 */

import { safeRead, safeWrite } from "../i18n";

/**
 * Everything the player knows how to draw.
 *
 * Adding one is a change here and in the drawing, which is what keeps a stored
 * arrangement from naming something nobody can draw.
 */
export const CONTROLS = [
  "back",
  "logo",
  "title",
  "play",
  "step_back",
  "step_on",
  "previous_chapter",
  "next_chapter",
  "previous_episode",
  "next_episode",
  "elapsed",
  "remaining",
  "ends_at",
  "info",
  "chapters",
  "cast",
  "episodes",
  "separator",
  "favourite",
  "subtitles",
  "audio",
  "volume",
  "settings",
  "corner",
  "fullscreen",
] as const;

export type Control = (typeof CONTROLS)[number];

/**
 * The places a control can sit.
 *
 * Four of them, and they are the four a player has: the strip across the top,
 * the two ends of the bar itself, and the row underneath it. Anything else
 * would be a place nobody looks.
 */
export const ZONES = [
  "top_left",
  "top_right",
  "before_bar",
  "after_bar",
  "bottom_left",
  "bottom_right",
] as const;

export type Zone = (typeof ZONES)[number];

export type Arrangement = Record<Zone, Control[]>;

/**
 * What a viewer gets before deciding anything.
 *
 * The transport on the left where a hand goes for it, what is being watched on
 * the right where a hand goes to change something, and the clock at the two
 * ends of the bar it belongs to.
 *
 * The top right is deliberately empty. The place exists, is drawn, and lays
 * itself out correctly with nothing in it; what goes there is a later
 * question, and a place added the day something needs it is a place that has
 * never been tested.
 */
export const DEFAULT_ARRANGEMENT: Arrangement = {
  top_left: ["back", "logo", "title"],
  top_right: [],
  before_bar: ["elapsed"],
  after_bar: ["remaining"],
  bottom_left: [
    "previous_episode",
    "previous_chapter",
    "step_back",
    "play",
    "step_on",
    "next_chapter",
    "next_episode",
    "ends_at",
  ],
  bottom_right: [
    // What the film is, in front of what is being done with it, with a line
    // between so the two read as two groups rather than one long row.
    "info",
    "chapters",
    "cast",
    "episodes",
    "separator",
    "favourite",
    "subtitles",
    "audio",
    "volume",
    "settings",
    "corner",
    "fullscreen",
  ],
};

const STORED = "melyxar.player.arrangement";

/** Whether a name is one this build knows how to draw. */
function isAControl(name: unknown): name is Control {
  return typeof name === "string" && (CONTROLS as readonly string[]).includes(name);
}

/**
 * Reads back an arrangement, keeping only what this build can draw.
 *
 * A stored arrangement outlives the build that wrote it: a control dropped
 * from a later version is still named in what somebody saved, and a control
 * added is in nobody's. So a name nobody can draw is passed over rather than
 * refused, and a place the stored one says nothing about keeps what everyone
 * gets. Losing one button is a button missing; refusing the whole thing is a
 * player with no controls at all.
 */
export function storedArrangement(): Arrangement {
  const raw = safeRead(STORED);
  if (!raw) {
    return DEFAULT_ARRANGEMENT;
  }
  try {
    const held = JSON.parse(raw) as Partial<Record<Zone, unknown>>;
    const kept = { ...DEFAULT_ARRANGEMENT };
    for (const zone of ZONES) {
      const named = held[zone];
      if (Array.isArray(named)) {
        kept[zone] = named.filter(isAControl);
      }
    }
    return kept;
  } catch {
    return DEFAULT_ARRANGEMENT;
  }
}

/** Keeps an arrangement for next time. */
export function rememberArrangement(arrangement: Arrangement) {
  safeWrite(STORED, JSON.stringify(arrangement));
}
