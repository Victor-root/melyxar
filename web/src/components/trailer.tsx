/*
 * A trailer, in a panel over the page it was opened from.
 *
 * One hosted on YouTube or Vimeo plays in the player that site offers other
 * pages, whole, with its own controls. One sitting next to the film plays in
 * the browser's own. Either way the page stays underneath, where it was, and
 * shutting the panel is being back on it.
 */

import { useState } from "react";
import { useSettings } from "../settings";
import { Modal } from "./modal";

/** Where a trailer plays from: another site's player, or a file here. */
export type Trailer = { embed: string } | { file: string };

export function TrailerDialog({
  trailer,
  title,
  onClose,
}: {
  trailer: Trailer;
  /** The work it is the trailer of. */
  title: string;
  onClose: () => void;
}) {
  const { t } = useSettings();
  const [failed, setFailed] = useState(false);
  const named = t("work.trailer_of", { title });

  return (
    <Modal title={named} onClose={onClose} className="modal-trailer">
      {"embed" in trailer ? (
        <iframe
          className="trailer-picture"
          src={trailer.embed}
          title={named}
          allow="autoplay; encrypted-media; picture-in-picture; fullscreen"
          allowFullScreen
          /* Where it is played from is said to the site, which some players
             refuse to start without. */
          referrerPolicy="strict-origin-when-cross-origin"
        />
      ) : failed ? (
        <p className="notice trailer-notice">{t("player.cannot_play")}</p>
      ) : (
        <video
          className="trailer-picture"
          src={trailer.file}
          controls
          autoPlay
          onError={() => setFailed(true)}
        />
      )}
    </Modal>
  );
}
