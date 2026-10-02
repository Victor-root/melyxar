/*
 * The page of what is playing, opened over the interface from the bar,
 * from wherever one is: the cover large, every control, and beside them the
 * queue or the words of the song.
 * Closed, it gives back the page it was opened over, just as it was.
 */

import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { asClock } from "../../clock";
import { CloseIcon } from "../../icons";
import { useIsAFilmOnScreen } from "../../on-screen";
import { BackIcon } from "../../player/icons";
import { ICON } from "../../player/sound";
import { useSettings } from "../../settings";
import { Cover } from "./bar";
import { HeartButton, Rail, Transport, Volume, Ways } from "./controls";
import { LyricsPanel } from "./lyrics-panel";
import { useNowPlayingPage } from "./opening";
import { useMusic } from "./player";

export function MusicNowPlaying() {
  const { t } = useSettings();
  const music = useMusic();
  const film = useIsAFilmOnScreen();
  const { song } = music;
  const nowPlaying = useNowPlayingPage();
  const shown = nowPlaying.marked && song !== null && !film;
  const [side, setSide] = useState<"queue" | "lyrics">("queue");

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

  if (!shown || !song) {
    return null;
  }
  const { queue } = music;

  return (
    <div className="music-now music-dark" role="dialog" aria-modal="true" aria-label={t("music.now_playing")}>
      <div className="music-now-glow" aria-hidden="true" />
      <div className="player-top">
        <div className="player-zone player-zone-top-left">
          <button type="button" className="player-button" onClick={close} aria-label={t("music.close_player")}>
            <BackIcon size={ICON} />
          </button>
          <span className="player-title">{song.title}</span>
        </div>
      </div>

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

        <section className="music-now-queue" aria-label={t(side === "queue" ? "music.queue" : "music.lyrics")}>
          <nav className="music-now-tabs">
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
          {side === "lyrics" ? (
            <LyricsPanel song={song.id} />
          ) : (
          <ol className="music-queue">
            {queue.order.map((place, at) => {
              const one = queue.songs[place];
              const here = at === queue.at;
              return (
                <li key={`${place}-${at}`} className={`music-queue-line${here ? " music-queue-here" : ""}${at < queue.at ? " music-queue-played" : ""}`}>
                  <button type="button" className="music-queue-song" onClick={() => music.jump(at)} aria-current={here ? "true" : undefined}>
                    <span className="music-queue-title">{one.title}</span>
                    <span className="music-queue-artists">{one.artists.map((artist) => artist.name).join(", ")}</span>
                  </button>
                  <span className="music-queue-length">{one.seconds === null ? "" : asClock(one.seconds)}</span>
                  {at > queue.at && (
                    <button type="button" className="player-button player-button-small music-queue-remove" onClick={() => music.remove(at)} aria-label={t("music.take_out", { title: one.title })}>
                      <CloseIcon size={14} />
                    </button>
                  )}
                </li>
              );
            })}
          </ol>
          )}
        </section>
      </div>

      <div className="player-bottom">
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
    </div>
  );
}
