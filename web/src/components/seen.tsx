/*
 * The one mark in the corner of a picture that says where somebody is with a
 * work.
 *
 * A banner folded over the top right corner rather than a badge sitting near
 * it: it belongs to the picture instead of floating on it, it cannot be
 * mistaken for a button that opens something, and it is read from across a
 * room, which is what a wall of cards needs.
 *
 * One element, not two. A series with episodes left shows how many; a series
 * with none left, and a film that has been watched, show a tick. They are the
 * same banner in the same corner and only what is inside it changes, because
 * they answer the same question and two marks that take turns in one corner
 * read as a mark that moves.
 *
 * At rest it appears only when it has something to say. A row where every card
 * wears one is a row of banners, and the one card that really was watched no
 * longer stands out, which is the only thing this is for.
 *
 * Under the pointer it becomes the button that marks and unmarks, and appears
 * greyed on the cards that had nothing to say: marking a film watched by hand
 * has to be reachable without opening a menu, and the grey says plainly that
 * this one has not been watched yet. Given nothing to do it stays a statement,
 * which is what the player's row of episodes needs.
 */

import { useSettings } from "../settings";
import { TickIcon } from "../icons";

/** How many figures the banner holds before it is widened, and before it is
 *  widened again. A tick and one or two figures sit in the corner as it is;
 *  three need a larger corner, and four a larger one still. */
const FITS_IN_THE_CORNER = 2;
const FITS_IN_A_LARGER_CORNER = 3;

export function SeenMark({
  watched,
  /** Episodes below this work, and how many of them are left. Both nothing
      for a film, which holds none. */
  episodes = 0,
  unwatched = 0,
  onPress,
  /** Whether the greyed button is drawn on a card that has nothing to say,
      which is once the pointer or the keyboard has reached it. Drawn on
      every card of a grid, invisible, it was four elements of each card the
      browser styled and laid out for nothing. */
  offered = false,
}: {
  watched: boolean;
  episodes?: number;
  unwatched?: number;
  onPress?: (watched: boolean) => void;
  offered?: boolean;
}) {
  const { t } = useSettings();
  const counting = episodes > 0 && unwatched > 0;
  const said = counting
    ? t("card.unwatched", { count: unwatched })
    : t(watched ? "card.mark_unwatched" : "card.mark_watched");
  const written = counting ? String(unwatched) : "";
  const inside = counting ? written : <TickIcon size={14} />;
  const marked = counting || watched;
  /* Widened for what does not fit, rather than widened for everything: the
     corner is a triangle, so what room there is narrows as it goes down, and
     a banner sized for four figures on every card is a banner too big for a
     tick. */
  const long =
    written.length > FITS_IN_A_LARGER_CORNER
      ? "longer"
      : written.length > FITS_IN_THE_CORNER
        ? "yes"
        : undefined;

  if (!onPress) {
    return marked ? (
      <span
        className="seen-mark seen-mark-on"
        data-long={long}
        title={said}
        aria-label={said}
      >
        <span className="seen-mark-said">{inside}</span>
      </span>
    ) : null;
  }
  if (!marked && !offered) {
    return null;
  }
  return (
    <button
      type="button"
      className={`seen-mark${marked ? " seen-mark-on" : ""}`}
      data-long={long}
      aria-pressed={counting ? undefined : watched}
      aria-label={said}
      title={said}
      onClick={(event) => {
        // The card behind it is one big link: everything drawn on top has to
        // say it is not part of it.
        event.preventDefault();
        event.stopPropagation();
        onPress(!watched);
      }}
    >
      <span className="seen-mark-said">{inside}</span>
    </button>
  );
}
