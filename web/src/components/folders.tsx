/*
 * Looking around the server's disk for a folder to make a library of.
 *
 * The one screen that sends a path to the server. It only ever asks what
 * folders are inside a folder: no file is ever named, here or on the other
 * side, and nothing is opened. What comes back for each folder is how many
 * videos sit directly inside it, which is the one number that answers "is this
 * the folder I meant" without naming a single film.
 */

import { useState } from "react";
import { api } from "../api";
import { refusalAbout, useAsked } from "../asking";
import { ChevronRightIcon, ChevronUpIcon, FolderIcon } from "../icons";
import { useSettings } from "../settings";
import { Modal } from "./modal";

export function FolderPicker({
  onPick,
  onClose,
}: {
  /** Called with the whole path of the folder somebody settled on. */
  onPick: (path: string) => void;
  onClose: () => void;
}) {
  const { t } = useSettings();
  /* Null until the server has said where the top of the tree is: asking for
     nothing is how somebody with nothing typed in starts. */
  const [asking, setAsking] = useState<string | null>(null);
  const {
    answer: listing,
    failure,
    waiting: busy,
  } = useAsked((signal) => api.folders(asking, signal), [asking]);
  const refused = failure && refusalAbout(failure, "library");

  return (
    <Modal
      title={t("folders.title")}
      onClose={onClose}
      footer={
        <>
          <button className="button" onClick={onClose}>
            {t("settings.cancel")}
          </button>
          <button
            className="button button-accent"
            disabled={!listing || busy}
            onClick={() => listing && onPick(listing.path)}
          >
            {t("folders.choose")}
          </button>
        </>
      }
    >
      <div className="browse">
        <div className="browse-where">
          <button
            className="button button-small"
            disabled={!listing?.parent || busy}
            onClick={() => setAsking(listing?.parent ?? null)}
            title={t("folders.up")}
          >
            <ChevronUpIcon size={16} />
            {t("folders.up")}
          </button>
          {/* The folder being looked in, whole. This is the one screen where a
              path is the subject rather than something to keep out of sight. */}
          <span className="browse-path">{listing?.path ?? ""}</span>
        </div>

        {refused && <p className="panel-notice panel-notice-trouble">{t(refused)}</p>}

        <div className="browse-list" aria-busy={busy}>
          {busy && <p className="empty-line">{t("folders.looking")}</p>}
          {listing && !busy && listing.folders.length === 0 && (
            <p className="empty-line">{t("folders.none")}</p>
          )}
          {!busy &&
            listing?.folders.map((folder) => (
              <button
                key={folder.path}
                className="browse-folder"
                onClick={() => setAsking(folder.path)}
              >
                <FolderIcon size={18} />
                <span className="browse-folder-name">{folder.name}</span>
                {folder.videos > 0 && (
                  <span className="browse-folder-videos">
                    {folder.more_videos
                      ? t("folders.videos_more", { count: folder.videos })
                      : t("folders.videos", { count: folder.videos })}
                  </span>
                )}
                <ChevronRightIcon size={16} />
              </button>
            ))}
        </div>

        {listing?.cut_short && <p className="panel-say">{t("folders.cut_short")}</p>}
      </div>
    </Modal>
  );
}
