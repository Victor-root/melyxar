/*
 * The lyrics of a song, for an administrator: where the ones it has come
 * from, and a search of LRCLIB by hand, one entry of which is taken as the
 * words of the song. Asked of the song's menu, the way the subtitles of a
 * film are asked of its own.
 *
 * Whatever changes here is read again by the lyrics on screen, so the words
 * taken are the words shown at once.
 */

import { useState } from "react";
import type { ReactNode } from "react";
import { useAccount } from "../account";
import { refusalOf, useAsked } from "../asking";
import { asClock } from "../clock";
import { Modal } from "../components/modal";
import { CloseIcon, DownloadIcon, SubtitlesIcon } from "../icons";
import { useSettings } from "../settings";
import { music } from "./api";
import type { LyricsOffer, Song } from "./api";
import { useMusicMarks } from "./marks";
import type { MenuLine } from "./song-menu";

/** How long without a line stamped is worth saying: the song runs on with
    nothing lit. */
const LONG_GAP_SECONDS = 12;

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
  const { t } = useSettings();
  const { lyricsHaveMoved } = useMusicMarks();
  const [again, setAgain] = useState(0);
  const current = useAsked((signal) => music.lyrics(song.id, signal), [song.id, again]);
  const [artist, setArtist] = useState(song.artists[0]?.name ?? "");
  const [title, setTitle] = useState(song.title);
  const [offers, setOffers] = useState<LyricsOffer[] | null>(null);
  const [busy, setBusy] = useState<number | "search" | null>(null);
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
                    {offer.longest_gap_seconds !== null && offer.longest_gap_seconds >= LONG_GAP_SECONDS && (
                      <span className="work-badge work-badge-warning" title={t("lyrics_dialog.gap_why")}>
                        {t("lyrics_dialog.gap", { seconds: offer.longest_gap_seconds })}
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
