/*
 * Every version of the lyrics written by hand for a song, the latest first:
 * when it was written, how long it is, how it opens, and whether it is the
 * one shown. A version set aside is put back in one press; deleting one is
 * the only way one is ever lost, so it is asked first.
 *
 * Shown in the window of the lyrics of a song and in the editor, which tell
 * it when the versions may have changed and are told when it changed them.
 */

import { useEffect, useState } from "react";
import { refusalOf } from "../asking";
import { Modal } from "../components/modal";
import { DeleteIcon, HistoryIcon } from "../icons";
import { readableDate } from "../readable";
import { useSettings } from "../settings";
import { music as server } from "./api";
import type { LyricsVersion } from "./api";

export function LyricsVersions({
  song,
  read,
  unsaved = false,
  saysWhenEmpty = false,
  onChanged,
}: {
  song: string;
  /** Moves whenever the versions may have changed elsewhere. */
  read: number;
  /** Changes not saved, which putting a version back would throw away. */
  unsaved?: boolean;
  /** Says there is none yet, rather than showing nothing. */
  saysWhenEmpty?: boolean;
  /** A version was put back or deleted. */
  onChanged: () => void;
}) {
  const { t, language } = useSettings();
  // Nothing yet while they are read.
  const [versions, setVersions] = useState<LyricsVersion[] | null>(null);
  const [again, setAgain] = useState(0);
  const [asking, setAsking] = useState<{ version: LyricsVersion; doing: "put back" | "delete" } | null>(null);
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);

  useEffect(() => {
    const stop = new AbortController();
    server.lyricsVersions(song, stop.signal).then(setVersions, () => {
      if (!stop.signal.aborted) {
        setVersions([]);
      }
    });
    return () => stop.abort();
  }, [song, read, again]);

  const act = async (version: LyricsVersion, doing: "put back" | "delete") => {
    setAsking(null);
    setBusy(true);
    setRefused(null);
    try {
      if (doing === "put back") {
        await server.useLyricsVersion(song, version.id);
      } else {
        await server.deleteLyricsVersion(song, version.id);
      }
      setAgain((count) => count + 1);
      onChanged();
    } catch (error) {
      setRefused(`lyrics_dialog.refused.${refusalOf(error)}`);
    } finally {
      setBusy(false);
    }
  };

  if (versions === null || (versions.length === 0 && !saysWhenEmpty)) {
    return null;
  }
  return (
    <section className="lyrics-versions" aria-label={t("lyrics_versions.title")}>
      <h3 className="lyrics-versions-title">
        <HistoryIcon size={16} />
        {t("lyrics_versions.title")}
      </h3>
      <p className="lyrics-versions-why">{t("lyrics_versions.why")}</p>
      {refused && <p className="notice">{t(refused)}</p>}
      {versions.length === 0 && <p className="lyrics-versions-none">{t("lyrics_versions.none")}</p>}
      <ol className="lyrics-versions-list">
        {versions.map((version) => (
          <li key={version.id} className={`lyrics-version${version.in_use ? " lyrics-version-in-use" : ""}`}>
            <span className="lyrics-version-words">
              <span className="lyrics-version-opening">{version.opening || t("lyrics_editor.break")}</span>
              <span className="lyrics-version-said">
                {[
                  readableDate(version.written_at, language),
                  t("lyrics_dialog.lines", { count: version.lines }),
                  t(version.stamped ? "lyrics_dialog.synced" : "lyrics_dialog.plain"),
                ]
                  .filter(Boolean)
                  .join(" · ")}
              </span>
            </span>
            {version.in_use ? (
              <span className="state-pill state-ok">{t("lyrics_versions.in_use")}</span>
            ) : (
              <button
                type="button"
                className="button button-small"
                disabled={busy}
                onClick={() => (unsaved ? setAsking({ version, doing: "put back" }) : void act(version, "put back"))}
              >
                {t("lyrics_versions.put_back")}
              </button>
            )}
            <button
              type="button"
              className="lyrics-line-button lyrics-line-remove"
              disabled={busy}
              title={t("lyrics_versions.delete")}
              aria-label={t("lyrics_versions.delete")}
              onClick={() => setAsking({ version, doing: "delete" })}
            >
              <DeleteIcon size={15} />
            </button>
          </li>
        ))}
      </ol>

      {asking && (
        <Modal
          title={t(asking.doing === "delete" ? "lyrics_versions.delete_title" : "lyrics_versions.put_back_title")}
          onClose={() => setAsking(null)}
          className="lyrics-editor-ask"
          footer={
            <>
              <button type="button" className="button button-quiet" onClick={() => setAsking(null)}>
                {t(asking.doing === "delete" ? "lyrics_versions.keep" : "lyrics_editor.leave_stay")}
              </button>
              <button type="button" className="button button-accent" onClick={() => void act(asking.version, asking.doing)}>
                {t(asking.doing === "delete" ? "lyrics_versions.delete_go" : "lyrics_versions.put_back")}
              </button>
            </>
          }
        >
          <p>{t(asking.doing === "delete" ? "lyrics_versions.delete_why" : "lyrics_versions.put_back_why")}</p>
        </Modal>
      )}
    </section>
  );
}
