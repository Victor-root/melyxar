/*
 * The page of what is playing, opened over the page one is on from the bar:
 * the cover large and beside it the queue or the words of the song, between
 * the header and the bar, which stay where they are and are the same ones
 * whether the page is open or not. On a phone the page takes the whole
 * screen, controls included.
 * Closed, it gives back the page it was opened over, just as it was.
 */

import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { useMediaQuery } from "../../media-query";
import { useIsAFilmOnScreen } from "../../on-screen";
import { BackIcon } from "../../player/icons";
import { ICON } from "../../player/sound";
import { useSettings } from "../../settings";
import { Cover } from "./bar";
import { HeartButton, Rail, Transport, Volume, Ways } from "./controls";
import { LyricsPanel } from "./lyrics-panel";
import { useLeaving } from "./leaving";
import { useNowPlayingPage } from "./opening";
import { useMusic } from "./player";
import { QueuePanel } from "./queue-panel";
import { BehindThePlayer } from "./spectrum";

/** How long the page takes to leave, which is what its way out lasts. */
const LEAVE_MS = 180;

export function MusicNowPlaying() {
  const { t } = useSettings();
  const music = useMusic();
  const film = useIsAFilmOnScreen();
  const { song } = music;
  const nowPlaying = useNowPlayingPage();
  const shown = nowPlaying.marked && song !== null && !film;
  const [side, setSide] = useState<"queue" | "lyrics">("queue");
  const onAPhone = useMediaQuery("(max-width: 760px)");
  const reduced = useMediaQuery("(prefers-reduced-motion: reduce)");
  const leaving = useLeaving(shown, song !== null && !film, reduced ? 0 : LEAVE_MS);

  const { marked, close, forget } = nowPlaying;
  useEffect(() => {
    if (!shown) {
      return;
    }
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        close();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [shown, close]);

  useEffect(() => {
    if (marked && song === null) {
      forget();
    }
  }, [marked, song, forget]);

  if ((!shown && !leaving) || !song) {
    return null;
  }

  return (
    <div className={`music-now music-dark${leaving ? " music-now-leaving" : ""}`} role="dialog" aria-modal="true" aria-label={t("music.now_playing")}>
      <div className="music-now-glow" aria-hidden="true" />
      {onAPhone && (
        <div className="player-top">
          <div className="player-zone player-zone-top-left">
            <button type="button" className="player-button" onClick={close} aria-label={t("music.close_player")}>
              <BackIcon size={ICON} />
            </button>
            <span className="player-title">{song.title}</span>
          </div>
        </div>
      )}

      <div className="music-now-body">
        <section className="music-now-playing">
          <Cover pictures={song.cover} large />
          <div className="music-now-words">
            <h2>{song.title}</h2>
            <p className="music-now-by">
              {song.artists.map((artist, index) => (
                <span key={artist.id}>
                  {index > 0 && ", "}
                  <Link to={`/music/artist/${artist.id}`} onClick={close}>
                    {artist.name}
                  </Link>
                </span>
              ))}
            </p>
            {song.album && (
              <p className="music-now-album">
                <Link to={`/music/album/${song.album.id}`} onClick={close}>
                  {song.album.name}
                </Link>
              </p>
            )}
          </div>
        </section>

        <nav className="music-now-tabs" aria-label={t("music.queue")}>
          {(["queue", "lyrics"] as const).map((one) => (
            <button
              key={one}
              type="button"
              className={`music-tab${side === one ? " music-tab-on" : ""}`}
              aria-current={side === one ? "true" : undefined}
              onClick={() => setSide(one)}
            >
              {t(one === "queue" ? "music.queue" : "music.lyrics")}
            </button>
          ))}
        </nav>

        <section className="music-now-queue" aria-label={t(side === "queue" ? "music.queue" : "music.lyrics")}>
          {side === "lyrics" ? <LyricsPanel song={song.id} /> : <QueuePanel />}
        </section>
      </div>

      {onAPhone && (
        <div className="player-bottom">
          <BehindThePlayer />
          <Rail music={music} />
          <div className="player-row">
            <div className="player-zone player-zone-bottom-left">
              <Transport music={music} />
            </div>
            <div className="player-zone player-zone-bottom-right">
              <HeartButton id={song.id} />
              <Volume music={music} />
              <Ways music={music} />
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
