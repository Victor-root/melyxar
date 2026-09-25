/*
 * DEBUG ONLY, TO BE REMOVED: a tiny button in the bottom right corner that
 * opens the technical journal over whatever is on screen, so it can be read,
 * filtered, copied and emptied without leaving the page, the film or the
 * filled screen being debugged. Not part of the interface: it goes once the
 * debugging it is for is over, with its styles in app.css and its words in
 * i18n.ts, all marked the same way.
 *
 * Shown to administrators only, since nobody else may read the journal.
 * Drawn on top of everything, the player included, and inside whatever
 * fills the screen when something does: a filled screen shows nothing of
 * the page outside it.
 */

import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { useAccount } from "../account";
import { JournalIcon } from "../icons";
import { TechnicalJournal } from "../pages/admin/journal";
import { useSettings } from "../settings";
import { Modal } from "./modal";

/** What fills the screen right now, or nothing. */
function useFillingTheScreen(): Element | null {
  const [filling, setFilling] = useState<Element | null>(() => document.fullscreenElement);
  useEffect(() => {
    const tell = () => setFilling(document.fullscreenElement);
    document.addEventListener("fullscreenchange", tell);
    return () => document.removeEventListener("fullscreenchange", tell);
  }, []);
  return filling;
}

export function DebugJournal() {
  const { t } = useSettings();
  const { account } = useAccount();
  const filling = useFillingTheScreen();
  const [layer, setLayer] = useState<HTMLDivElement | null>(null);
  const [open, setOpen] = useState(false);

  if (!account?.is_administrator) {
    return null;
  }
  return createPortal(
    <div className="debug-layer" ref={setLayer}>
      <button
        className="debug-fab"
        onClick={() => setOpen(true)}
        aria-label={t("debug.journal")}
        title={t("debug.journal")}
      >
        <JournalIcon size={12} />
      </button>
      {open && layer && (
        <Modal
          title={t("admin.journal_lines")}
          onClose={() => setOpen(false)}
          className="debug-journal"
          into={layer}
        >
          <TechnicalJournal bare />
        </Modal>
      )}
    </div>,
    filling ?? document.body,
  );
}
