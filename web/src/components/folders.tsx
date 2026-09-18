/*
 * Looking around the server's disk for a folder to make a library of.
 *
 * The one screen that sends a path to the server. It only ever asks what
 * folders are inside a folder: no file is ever named, here or on the other
 * side, and nothing is opened. What comes back for each folder is how many
 * videos sit directly inside it, which is the one number that answers "is this
 * the folder I meant" without naming a single film.
 */

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "../api";
import type { Listing } from "../api";
import { useSettings } from "../settings";

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
  const [listing, setListing] = useState<Listing | null>(null);
  const [asking, setAsking] = useState<string | null>(null);
  const [refused, setRefused] = useState<string | null>(null);
  const [busy, setBusy] = useState(true);

  const look = useCallback(async (path: string | null, signal?: AbortSignal) => {
    setBusy(true);
    setRefused(null);
    try {
      setListing(await api.folders(path, signal));
    } catch (error) {
      if (error instanceof DOMException) {
        return;
      }
      setRefused(
        error instanceof ApiError && error.reason
          ? `refused.library.${error.reason}`
          : "refused.generic",
      );
    } finally {
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    const controller = new AbortController();
    look(asking, controller.signal);
    return () => controller.abort();
  }, [asking, look]);

  return (
    <div className="picker">
      <div className="picker-head">
        {/* The folder being looked in, whole. This is the one screen where a
            path is the subject rather than something to keep out of sight. */}
        <span className="picker-path">{listing?.path ?? ""}</span>
        <button className="button button-small" onClick={onClose}>
          {t("folders.close")}
        </button>
      </div>

      {refused && <p className="notice">{t(refused)}</p>}

      <div className="picker-actions">
        <button
          className="button button-small"
          disabled={!listing?.parent || busy}
          onClick={() => setAsking(listing?.parent ?? null)}
        >
          {t("folders.up")}
        </button>
        <button
          className="button button-small button-accent"
          disabled={!listing || busy}
          onClick={() => listing && onPick(listing.path)}
        >
          {t("folders.choose")}
        </button>
      </div>

      {busy && <p className="notice notice-faint">{t("folders.looking")}</p>}

      {listing && !busy && listing.folders.length === 0 && (
        <p className="notice notice-faint">{t("folders.none")}</p>
      )}

      <div className="picker-list">
        {listing?.folders.map((folder) => (
          <button
            key={folder.path}
            className="picker-folder"
            onClick={() => setAsking(folder.path)}
          >
            <span className="picker-folder-name">{folder.name}</span>
            {folder.videos > 0 && (
              <span className="picker-folder-videos">
                {folder.more_videos
                  ? t("folders.videos_more", { count: folder.videos })
                  : t("folders.videos", { count: folder.videos })}
              </span>
            )}
          </button>
        ))}
      </div>

      {listing?.cut_short && <p className="notice notice-faint">{t("folders.cut_short")}</p>}
    </div>
  );
}
