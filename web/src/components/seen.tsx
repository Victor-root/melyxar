/*
 * The mark that says a work has been watched.
 *
 * One shape everywhere: the player's row of episodes wears it, every card in
 * the interface wears it, and the day it changes it changes in both at once.
 * It was drawn twice before, and the two did not look alike.
 *
 * A corner of the picture rather than a badge floating over it: it belongs to
 * the still it sits on, it never covers what the still is showing, and it
 * reads at a glance across a whole row without being aimed at.
 *
 * Given something to do, it becomes the button that marks and unmarks; given
 * nothing, it is a statement, which is what the player needs.
 */

import { useSettings } from "../settings";
import { TickIcon } from "../icons";

export function SeenMark({
  watched,
  onPress,
}: {
  watched: boolean;
  /** What pressing it does, where it may be pressed at all. */
  onPress?: (watched: boolean) => void;
}) {
  const { t } = useSettings();
  const said = t(watched ? "card.mark_unwatched" : "card.mark_watched");

  if (!onPress) {
    return (
      <span
        className="seen-mark seen-mark-on"
        title={t("work.watched")}
        aria-label={t("work.watched")}
      >
        <TickIcon size={14} />
      </span>
    );
  }
  return (
    <button
      type="button"
      className={`seen-mark${watched ? " seen-mark-on" : ""}`}
      aria-pressed={watched}
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
      <TickIcon size={14} />
    </button>
  );
}
