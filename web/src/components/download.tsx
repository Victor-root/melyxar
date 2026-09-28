/*
 * Keeping a copy of a film or an episode as it lies on the disk.
 *
 * A title with one file is handed over at once, without a question. One with
 * several versions asks which: they can differ by gigabytes, and the one
 * wanted on a telephone is rarely the one wanted on a television. The server
 * refuses anybody not allowed to download, whatever this shows.
 */

import { useEffect, useRef } from "react";
import { api } from "../api";
import type { Version } from "../api";
import { useAsked } from "../asking";
import { amountOfData } from "../readable";
import { useSettings } from "../settings";
import { Modal } from "./modal";

/** Starts the browser's own download of one copy. */
function download(version: Version) {
  const link = document.createElement("a");
  link.href = api.downloadAddress(version.id);
  link.download = "";
  link.click();
}

export function DownloadDialog({
  workId,
  title,
  onClose,
}: {
  workId: string;
  title: string;
  onClose: () => void;
}) {
  const { t, language } = useSettings();
  const work = useAsked((signal) => api.work(workId, signal), [workId]);
  const versions = (work.answer?.versions ?? []).filter((version) => !version.missing);
  const single = work.answer !== null && versions.length === 1 ? versions[0] : null;

  /* Once, however often the screen around it is drawn again before this
     panel is taken away. */
  const handed = useRef(false);
  useEffect(() => {
    if (single && !handed.current) {
      handed.current = true;
      download(single);
      onClose();
    }
  }, [single, onClose]);

  if (work.answer === null || single) {
    return null;
  }

  return (
    <Modal title={t("download.title", { title })} onClose={onClose}>
      {versions.length === 0 ? (
        <p className="notice">{t("download.none")}</p>
      ) : (
        <>
          <p className="identify-said">{t("download.which")}</p>
          <ul className="download-versions">
            {versions.map((version) => (
              <li key={version.id}>
                <button
                  type="button"
                  className="download-version"
                  onClick={() => {
                    download(version);
                    onClose();
                  }}
                >
                  <span>{version.summary}</span>
                  <span className="download-size">{amountOfData(version.size_bytes, language)}</span>
                </button>
              </li>
            ))}
          </ul>
        </>
      )}
    </Modal>
  );
}
