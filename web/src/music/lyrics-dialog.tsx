/*
 * The lyrics of a song, for an administrator: where the ones it has come
 * from, and a search of LRCLIB by hand, one entry of which is taken as the
 * words of the song. Asked of the song's menu, the way the subtitles of a
 * film are asked of its own.
 *
 * Whatever changes here is read again by the lyrics on screen, so the words
 * taken are the words shown at once.
 */

import { useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { useAccount } from "../account";
import { refusalOf, useAsked } from "../asking";
import { asClock } from "../clock";
import { Modal } from "../components/modal";
import { CloseIcon, DownloadIcon, SubtitlesIcon } from "../icons";
import { outOfAHundred, shiftInSeconds } from "../readable";
import { useRunning } from "../running";
import { useSettings } from "../settings";
import { music } from "./api";
import type { LyricsOffer, Song, SongLyrics } from "./api";
import { offersMissingLines } from "./lyrics-offers";
import { useMusicMarks } from "./marks";
import type { MenuLine } from "./song-menu";

/** The line of the menu and the window it opens, for one song and an
    administrator only. */
export function useSongLyrics(songs: Song[]): { lines: MenuLine[]; dialog: ReactNode } {
  const { t } = useSettings();
  const { account } = useAccount();
  const [asking, setAsking] = useState(false);
  const song = songs.length === 1 ? songs[0] : null;
  if (!song || account?.is_administrator !== true) {
    return { lines: [], dialog: null };
  }
  return {
    lines: [{ key: "lyrics", said: t("lyrics_dialog.menu"), mark: <SubtitlesIcon size={17} />, act: () => setAsking(true) }],
    dialog: asking ? <LyricsDialog song={song} onClose={() => setAsking(false)} /> : null,
  };
}

function LyricsDialog({ song, onClose }: { song: Song; onClose: () => void }) {
  const { t, language } = useSettings();
  const { lyricsHaveMoved } = useMusicMarks();
  const { jobs, watch, finished } = useRunning();
  const [again, setAgain] = useState(0);
  const current = useAsked((signal) => music.lyrics(song.id, signal), [song.id, again]);
  const [artist, setArtist] = useState(song.artists[0]?.name ?? "");
  const [title, setTitle] = useState(song.title);
  const [offers, setOffers] = useState<LyricsOffer[] | null>(null);
  const [busy, setBusy] = useState<number | "search" | null>(null);
  const [listenAsked, setListenAsked] = useState(false);
  const [said, setSaid] = useState<string | null>(null);
  const [refused, setRefused] = useState<string | null>(null);

  const attempt = async (doing: () => Promise<void>) => {
    setRefused(null);
    try {
      await doing();
    } catch (error) {
      setRefused(`lyrics_dialog.refused.${refusalOf(error)}`);
    }
    setBusy(null);
  };

  const search = () => {
    setBusy("search");
    setSaid(null);
    void attempt(async () => {
      setOffers(await music.lyricsOffers(song.id, artist, title));
    });
  };

  const take = (offer: LyricsOffer) => {
    setBusy(offer.id);
    setSaid(null);
    void attempt(async () => {
      await music.takeLyrics(song.id, offer.id);
      setSaid(t("lyrics_dialog.taken"));
      setAgain((count) => count + 1);
      lyricsHaveMoved();
    });
  };

  const forget = () =>
    void attempt(async () => {
      await music.forgetLyrics(song.id);
      setSaid(t("lyrics_dialog.forgotten"));
      setAgain((count) => count + 1);
      lyricsHaveMoved();
    });

  const synchronise = () => {
    setSaid(null);
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

  const missingLines = offersMissingLines(offers ?? []);
  const lyrics = current.answer;
  return (
    <Modal title={t("lyrics_dialog.title", { title: song.title })} onClose={onClose} className="modal-subtitles">
      <h3 className="subtitles-heading">{t("lyrics_dialog.current")}</h3>
      {!current.waiting && (
        <ul className="subtitles-lines">
          <li className="subtitles-line">
            <span className="subtitles-release">
              {lyrics === null
                ? t("music.lyrics_none")
                : t(lyrics.instrumental && lyrics.plain === "" ? "music.lyrics_instrumental" : `lyrics_dialog.origin.${lyrics.source}`)}
            </span>
            {lyrics !== null && lyrics.plain !== "" && (
              <span className="subtitles-origin">
                {t(lyrics.lines.length > 0 ? "lyrics_dialog.synced" : "lyrics_dialog.plain")}
              </span>
            )}
            {lyrics === null && (
              <button type="button" className="button button-small" title={t("lyrics_dialog.look_again_why")} onClick={forget}>
                {t("lyrics_dialog.look_again")}
              </button>
            )}
            {lyrics?.source === "online" && (
              <button
                type="button"
                className="subtitles-off"
                aria-label={t("lyrics_dialog.forget")}
                title={t("lyrics_dialog.forget")}
                onClick={forget}
              >
                <CloseIcon size={15} />
              </button>
            )}
          </li>
        </ul>
      )}

      {lyrics !== null && lyrics.lines.length > 0 && (
        <>
          <h3 className="subtitles-heading">{t("lyrics_dialog.sync")}</h3>
          <p className="settings-why">{t("lyrics_dialog.sync_why")}</p>
          <ul className="subtitles-lines">
            <li className="subtitles-line">
              <span className="subtitles-release">
                {listeningRuns
                  ? t("lyrics_dialog.sync_running")
                  : lyrics.synchronised
                    ? syncSaid(t, language, lyrics.synchronised)
                    : ""}
              </span>
              {lyrics.synchronised?.conclusion === "aligned" && !listeningRuns ? (
                <button
                  type="button"
                  className="subtitles-off"
                  aria-label={t("lyrics_dialog.sync_undo")}
                  title={t("lyrics_dialog.sync_undo")}
                  disabled={busy !== null}
                  onClick={unsynchronise}
                >
                  <CloseIcon size={15} />
                </button>
              ) : (
                <button
                  type="button"
                  className="button button-small"
                  disabled={busy !== null || listeningRuns}
                  onClick={synchronise}
                >
                  {t("lyrics_dialog.sync_run")}
                </button>
              )}
            </li>
          </ul>
          {listeningRuns && listening?.ratio != null && (
            <div className="job-card-progress">
              <span className="meter">
                <span className="meter-fill" style={{ width: `${outOfAHundred(listening.ratio)}%` }} />
              </span>
              <span className="job-card-count">{outOfAHundred(listening.ratio)} %</span>
            </div>
          )}
        </>
      )}

      <h3 className="subtitles-heading">{t("lyrics_dialog.search")}</h3>
      <form
        className="subtitles-search"
        onSubmit={(event) => {
          event.preventDefault();
          if (busy === null && title.trim() !== "") {
            search();
          }
        }}
      >
        <input
          className="field-line"
          value={artist}
          placeholder={t("lyrics_dialog.artist")}
          aria-label={t("lyrics_dialog.artist")}
          onChange={(event) => {
            setArtist(event.target.value);
            setOffers(null);
          }}
        />
        <input
          className="field-line"
          value={title}
          placeholder={t("lyrics_dialog.song")}
          aria-label={t("lyrics_dialog.song")}
          onChange={(event) => {
            setTitle(event.target.value);
            setOffers(null);
          }}
        />
        <button type="submit" className="button button-small button-accent" disabled={busy !== null || title.trim() === ""}>
          {t("online_subtitles.look")}
        </button>
      </form>
      {refused && <p className="notice">{t(refused)}</p>}
      {said && <p className="panel-notice panel-notice-ok">{said}</p>}
      {offers?.length === 0 && <p className="settings-why">{t("lyrics_dialog.nothing")}</p>}
      {offers && offers.length > 0 && (
        <ul className="subtitles-lines subtitles-offers">
          {offers.map((offer) => (
            <li key={offer.id} className="subtitles-line">
              <span className="subtitles-release">
                {offer.artist} · {offer.title}
                {offer.album ? ` · ${offer.album}` : ""}
                {offer.seconds === null ? "" : ` · ${asClock(offer.seconds)}`}
              </span>
              <span className="subtitles-marks">
                {offer.instrumental ? (
                  <span className="work-badge">{t("lyrics_dialog.instrumental")}</span>
                ) : (
                  <>
                    <span className={`work-badge${offer.synced ? " subtitles-fits" : ""}`}>
                      {t(offer.synced ? "lyrics_dialog.synced" : "lyrics_dialog.plain")}
                    </span>
                    {offer.synced && <span>{t("lyrics_dialog.lines", { count: offer.synced_lines })}</span>}
                    {missingLines.has(offer.id) && (
                      <span className="work-badge work-badge-warning" title={t("lyrics_dialog.gap_why")}>
                        {t("lyrics_dialog.gap", { seconds: offer.longest_gap_seconds ?? 0 })}
                      </span>
                    )}
                  </>
                )}
              </span>
              <button type="button" className="button button-small" disabled={busy !== null} onClick={() => take(offer)}>
                <DownloadIcon size={15} />
                {t("lyrics_dialog.take")}
              </button>
            </li>
          ))}
        </ul>
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
