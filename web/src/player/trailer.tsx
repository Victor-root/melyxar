/*
 * A trailer sitting next to the film.
 *
 * Deliberately not the film player. A trailer has no position to come back to,
 * no soundtrack to choose, no subtitles and no session: it is two minutes long
 * and it is watched once. Everything the film player does would be machinery
 * standing idle here, and the reasons it exists would stop being visible.
 *
 * A trailer the browser cannot open says so rather than showing a black
 * rectangle. There is no conversion behind this: a file the server put there
 * itself is almost always one a browser opens, and a whole engine for the
 * exception would cost more than it saves.
 */

import { useEffect, useState } from "react";
import { useSettings } from "../settings";
import "./player.css";

export function TrailerPlayer({
  url,
  title,
  onClose,
}: {
  url: string;
  title: string;
  onClose: () => void;
}) {
  const { t } = useSettings();
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div className="player-trailer" role="dialog" aria-label={title}>
      <div className="player-trailer-bar">
        <button className="button" onClick={onClose}>
          {t("player.close")}
        </button>
        <span className="player-trailer-title">{title}</span>
        <span className="fact">{t("work.trailer")}</span>
      </div>

      {failed ? (
        <p className="player-notice">{t("player.cannot_play")}</p>
      ) : (
        <video src={url} controls autoPlay onError={() => setFailed(true)} />
      )}
    </div>
  );
}
