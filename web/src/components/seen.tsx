/*
 * The one badge in the corner of a picture that says where somebody is with
 * a work.
 *
 * One element, not two. A series with episodes left shows how many; a series
 * with none left, and a film that has been watched, show a tick. They are the
 * same badge in the same corner and only what is inside it changes, because
 * they answer the same question and two badges that take turns in one corner
 * read as a badge that moves.
 *
 * At rest it appears only when it has something to say. A row where every
 * card wears an empty circle is a row of empty circles, and the one card that
 * really was watched no longer stands out, which is the only thing the badge
 * is for.
 *
 * Under the pointer it becomes the button that marks and unmarks, and appears
 * on cards that had nothing to say, since marking a film watched by hand has
 * to be reachable without opening a menu. Given nothing to do it stays a
 * statement, which is what the player's row of episodes needs.
 */

import { useSettings } from "../settings";
import { TickIcon } from "../icons";

export function SeenMark({
  watched,
  /** Episodes below this work, and how many of them are left. Both nothing
      for a film, which holds none. */
  episodes = 0,
  unwatched = 0,
  onPress,
}: {
  watched: boolean;
  episodes?: number;
  unwatched?: number;
  onPress?: (watched: boolean) => void;
}) {
  const { t } = useSettings();
  const counting = episodes > 0 && unwatched > 0;
  const said = counting
    ? t("card.unwatched", { count: unwatched })
    : t(watched ? "card.mark_unwatched" : "card.mark_watched");
  const inside = counting ? unwatched : <TickIcon size={15} />;
  const marked = counting || watched;

  if (!onPress) {
    return marked ? (
      <span className="seen-mark seen-mark-on" title={said} aria-label={said}>
        {inside}
      </span>
    ) : null;
  }
  return (
    <button
      type="button"
      className={`seen-mark${marked ? " seen-mark-on" : ""}`}
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
      {inside}
    </button>
  );
}
