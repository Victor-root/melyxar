/*
 * The lyrics of a song, for an administrator: where the ones it has come
 * from, and a search of LRCLIB by hand, one entry of which is taken as the
 * words of the song. Asked of the song's menu, the way the subtitles of a
 * film are asked of its own.
 *
 * Laid out as the search of the requests is: the song on top with the length
 * of its file to the hundredth, a search that asks as typing rests, and the
 * entries under it, nearest to that length first. Only the entries scroll.
 *
 * Whatever changes here is read again by the lyrics on screen, so the words
 * taken are the words shown at once. The lyrics are written again by hand in
 * the editor, opened from here or straight from the menu.
 */

import { useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { useAccount } from "../account";
import { refusalOf, useAsked } from "../asking";
import { asPreciseClock } from "../clock";
import { Modal } from "../components/modal";
import { CloseIcon, DownloadIcon, EditIcon, SearchIcon, SubtitlesIcon } from "../icons";
import { outOfAHundred, shiftInSeconds } from "../readable";
import { useRunning } from "../running";
import { useSettings } from "../settings";
import { music } from "./api";
import type { LyricsOffer, Song, SongLyrics } from "./api";
import { LyricsEditor } from "./lyrics-editor";
import { SAME_LENGTH_MS, lengthGap, offersMissingLines } from "./lyrics-offers";
import { useMusicMarks } from "./marks";
import type { MenuLine } from "./song-menu";
import { Cover } from "./songs";

/** How long typing rests before LRCLIB is asked. */
const ASKED_AFTER_MS = 500;

/** The lines of the menu and the windows they open, for one song and an
    administrator only. */
export function useSongLyrics(songs: Song[]): { lines: MenuLine[]; dialog: ReactNode } {
  const { t } = useSettings();
  const { account } = useAccount();
  const [asking, setAsking] = useState<"choose" | "edit" | null>(null);
  const song = songs.length === 1 ? songs[0] : null;
  if (!song || account?.is_administrator !== true) {
    return { lines: [], dialog: null };
  }
  const close = () => setAsking(null);
  return {
    lines: [
      { key: "lyrics", said: t("lyrics_dialog.menu"), mark: <SubtitlesIcon size={17} />, act: () => setAsking("choose") },
      { key: "lyrics_editor", said: t("lyrics_editor.menu"), mark: <EditIcon size={17} />, act: () => setAsking("edit") },
    ],
    dialog:
      asking === "choose" ? (
        <LyricsDialog song={song} onClose={close} />
      ) : asking === "edit" ? (
        <LyricsEditor song={song} onClose={close} />
      ) : null,
  };
}

function LyricsDialog({ song, onClose }: { song: Song; onClose: () => void }) {
  const { t, language } = useSettings();
  const { lyricsHaveMoved } = useMusicMarks();
  const { jobs, watch, finished } = useRunning();
  const [again, setAgain] = useState(0);
  const current = useAsked((signal) => music.lyrics(song.id, signal), [song.id, again]);
  const [typed, setTyped] = useState({ artist: song.artists[0]?.name ?? "", title: song.title });
  const [words, setWords] = useState(typed);
  const [busy, setBusy] = useState<number | null>(null);
  const [taken, setTaken] = useState<number | null>(null);
  const [listenAsked, setListenAsked] = useState(false);
  const [said, setSaid] = useState<string | null>(null);
  const [refused, setRefused] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);

  // LRCLIB is asked once typing rests, and at once on Enter.
  useEffect(() => {
    const resting = window.setTimeout(() => setWords(typed), ASKED_AFTER_MS);
    return () => window.clearTimeout(resting);
  }, [typed]);
  const offers = useAsked(
    (signal) =>
      words.title.trim() === ""
        ? Promise.resolve([])
        : music.lyricsOffers(song.id, words.artist.trim(), words.title.trim(), signal),
    [song.id, words.artist, words.title],
  );

  const attempt = async (doing: () => Promise<void>) => {
    setRefused(null);
    setSaid(null);
    try {
      await doing();
    } catch (error) {
      setRefused(`lyrics_dialog.refused.${refusalOf(error)}`);
    }
    setBusy(null);
  };

  const take = (offer: LyricsOffer) => {
    setBusy(offer.id);
    void attempt(async () => {
      await music.takeLyrics(song.id, offer.id);
      setTaken(offer.id);
      setSaid(t("lyrics_dialog.taken"));
      setAgain((count) => count + 1);
      lyricsHaveMoved();
    });
  };

  const forget = () =>
    void attempt(async () => {
      await music.forgetLyrics(song.id);
      setTaken(null);
      setSaid(t("lyrics_dialog.forgotten"));
      setAgain((count) => count + 1);
      lyricsHaveMoved();
    });

  const synchronise = () => {
    setListenAsked(true);
    void attempt(async () => {
      try {
        await music.synchroniseLyrics(song.id);
        watch();
      } catch (error) {
        setListenAsked(false);
        throw error;
      }
    });
  };

  const unsynchronise = () =>
    void attempt(async () => {
      await music.unsynchroniseLyrics(song.id);
      setSaid(t("lyrics_dialog.sync_undone"));
      setAgain((count) => count + 1);
      lyricsHaveMoved();
    });

  /* The listening is a job of the server's, followed with the others: when the
     last one ends the lyrics are read again, here and wherever they are shown. */
  const listening = jobs.find((job) => job.kind === "line_up_lyrics" && job.target === song.id) ?? null;
  const listeningRuns = listenAsked || listening !== null;
  const lastFinished = useRef(finished);
  useEffect(() => {
    if (finished !== lastFinished.current) {
      lastFinished.current = finished;
      setListenAsked(false);
      setAgain((count) => count + 1);
      lyricsHaveMoved();
    }
  }, [finished, lyricsHaveMoved]);

  const found = offers.answer ?? [];
  const missingLines = offersMissingLines(found);
  // A queue kept by the browser from before songs carried it has none.
  const fileLength = song.duration_ms ?? null;
  const lyrics = current.answer;
  const stamped = lyrics !== null && lyrics.lines.length > 0;
  const notice = refused ?? (offers.failure ? `lyrics_dialog.refused.${refusalOf(offers.failure)}` : null);

  return (
    <Modal title={t("lyrics_dialog.title", { title: song.title })} onClose={onClose} className="finder">
      <section className="finder-head lyrics-song" aria-label={t("lyrics_dialog.current")}>
        <Cover song={song} />
        <div className="finder-head-words">
          <h3 className="finder-head-title">{song.title}</h3>
          <span className="finder-head-by">
            {[song.artists.map((artist) => artist.name).join(", "), song.album?.name]
              .filter((part) => part)
              .join(" · ")}
          </span>
          {!current.waiting && (
            <div className="finder-head-state">
              {lyrics === null ? (
                <span className="state-pill state-attention">{t("music.lyrics_none")}</span>
              ) : lyrics.instrumental && lyrics.plain === "" ? (
                <span className="state-pill state-news">{t("music.lyrics_instrumental")}</span>
              ) : (
                <>
                  <span className="state-pill state-news">{t(`lyrics_dialog.origin.${lyrics.source}`)}</span>
                  <span className={`state-pill${stamped ? " state-ok" : " finder-pill-plain"}`}>
                    {t(stamped ? "lyrics_dialog.synced" : "lyrics_dialog.plain")}
                  </span>
                </>
              )}
              {stamped && !listeningRuns && lyrics.synchronised && (
                <span className="finder-head-note">{syncSaid(t, language, lyrics.synchronised)}</span>
              )}
              {listeningRuns && <span className="finder-head-note">{t("lyrics_dialog.sync_running")}</span>}
            </div>
          )}
          {listeningRuns && listening?.ratio != null && (
            <span className="meter lyrics-song-meter">
              <span className="meter-fill" style={{ width: `${outOfAHundred(listening.ratio)}%` }} />
            </span>
          )}
        </div>
        <div className="finder-head-side">
          {fileLength !== null && (
            <span className="finder-figure">
              <span className="finder-figure-name">{t("lyrics_dialog.file_length")}</span>
              <span className="finder-figure-value">{asPreciseClock(fileLength / 1000)}</span>
            </span>
          )}
          <div className="finder-head-actions">
            {!current.waiting && (
              <button
                type="button"
                className="button button-small"
                title={t("lyrics_editor.menu")}
                onClick={() => setEditing(true)}
              >
                <EditIcon size={15} />
                {t("lyrics_editor.open")}
              </button>
            )}
            {stamped &&
              (lyrics.synchronised?.conclusion === "aligned" && !listeningRuns ? (
                <button
                  type="button"
                  className="button button-small"
                  title={t("lyrics_dialog.sync_undo")}
                  disabled={busy !== null}
                  onClick={unsynchronise}
                >
                  {t("lyrics_dialog.sync_undo_short")}
                </button>
              ) : (
                <button
                  type="button"
                  className="button button-small"
                  title={t("lyrics_dialog.sync_why")}
                  disabled={busy !== null || listeningRuns}
                  onClick={synchronise}
                >
                  {t("lyrics_dialog.sync_run")}
                </button>
              ))}
            {lyrics === null && !current.waiting && (
              <button type="button" className="button button-small" title={t("lyrics_dialog.look_again_why")} onClick={forget}>
                {t("lyrics_dialog.look_again")}
              </button>
            )}
            {lyrics?.source === "online" && (
              <button
                type="button"
                className="finder-forget"
                aria-label={t("lyrics_dialog.forget")}
                title={t("lyrics_dialog.forget")}
                onClick={forget}
              >
                <CloseIcon size={16} />
              </button>
            )}
          </div>
        </div>
      </section>

      <form
        className="search-pill finder-search lyrics-search"
        role="search"
        aria-label={t("lyrics_dialog.search")}
        onSubmit={(event) => {
          event.preventDefault();
          setWords(typed);
        }}
      >
        <SearchIcon size={20} />
        <input
          value={typed.artist}
          placeholder={t("lyrics_dialog.artist")}
          aria-label={t("lyrics_dialog.artist")}
          onChange={(event) => setTyped((was) => ({ ...was, artist: event.target.value }))}
        />
        <span className="lyrics-search-between" aria-hidden="true" />
        <input
          value={typed.title}
          placeholder={t("lyrics_dialog.song")}
          aria-label={t("lyrics_dialog.song")}
          onChange={(event) => setTyped((was) => ({ ...was, title: event.target.value }))}
        />
        {/* Two fields need a button of the form for Enter to send it. */}
        <button type="submit" hidden />
      </form>

      {notice && <p className="notice">{t(notice)}</p>}
      {said && <p className="panel-notice panel-notice-ok">{said}</p>}

      <div className="finder-offers" aria-busy={offers.waiting}>
        {offers.waiting && found.length === 0 && <p className="finder-quiet">{t("lyrics_dialog.searching")}</p>}
        {!offers.waiting && !offers.failure && found.length === 0 && words.title.trim() !== "" && (
          <p className="finder-quiet">{t("lyrics_dialog.nothing")}</p>
        )}
        {found.map((offer) => {
          const gap = lengthGap(offer, fileLength);
          return (
            <article key={offer.id} className={`finder-offer${taken === offer.id ? " finder-offer-taken" : ""}`}>
              <div className="finder-offer-lead">
                <span className="finder-offer-lead-main">
                  {offer.duration_ms === null ? "?" : asPreciseClock(offer.duration_ms / 1000)}
                </span>
                {gap !== null && (
                  <span
                    className={`finder-offer-lead-note${Math.abs(gap) <= SAME_LENGTH_MS ? " state-ok" : " state-attention"}`}
                    title={t("lyrics_dialog.length_gap_why")}
                  >
                    {Math.round(gap / 100) === 0 ? t("lyrics_dialog.same_length") : `${shiftInSeconds(gap, language)} s`}
                  </span>
                )}
              </div>
              <div className="finder-offer-words">
                <span className="finder-offer-title">{offer.title}</span>
                <span className="finder-offer-by">
                  {offer.artist}
                  {offer.album ? ` · ${offer.album}` : ""}
                </span>
                <span className="finder-offer-marks">
                  {offer.instrumental ? (
                    <span className="state-pill state-news">{t("lyrics_dialog.instrumental")}</span>
                  ) : (
                    <>
                      <span className={`state-pill${offer.synced ? " state-ok" : " finder-pill-plain"}`}>
                        {t(offer.synced ? "lyrics_dialog.synced" : "lyrics_dialog.plain")}
                      </span>
                      {offer.synced && <span>{t("lyrics_dialog.lines", { count: offer.synced_lines })}</span>}
                      {missingLines.has(offer.id) && (
                        <span className="state-pill state-attention" title={t("lyrics_dialog.gap_why")}>
                          {t("lyrics_dialog.gap", { seconds: offer.longest_gap_seconds ?? 0 })}
                        </span>
                      )}
                    </>
                  )}
                </span>
              </div>
              {taken === offer.id ? (
                <span className="state-pill state-ok">{t("lyrics_dialog.in_use")}</span>
              ) : (
                <button
                  type="button"
                  className="button button-small"
                  disabled={busy !== null}
                  onClick={() => take(offer)}
                >
                  <DownloadIcon size={15} />
                  {t("lyrics_dialog.take")}
                </button>
              )}
            </article>
          );
        })}
      </div>

      {editing && (
        <LyricsEditor
          song={song}
          onClose={() => setEditing(false)}
          onChanged={() => {
            setTaken(null);
            setAgain((count) => count + 1);
          }}
        />
      )}
    </Modal>
  );
}

/** What came of the last attempt, as a sentence. */
function syncSaid(
  t: ReturnType<typeof useSettings>["t"],
  language: string,
  attempt: NonNullable<SongLyrics["synchronised"]>,
): string {
  if (attempt.conclusion === "aligned") {
    return t("lyrics_dialog.sync_on", { lines: attempt.lines_moved, seconds: shiftInSeconds(attempt.shift_ms, language) });
  }
  return t(`lyrics_dialog.sync_result.${attempt.conclusion}`);
}
