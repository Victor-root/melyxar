/*
 * The page of what is playing, opened over the interface from the bar,
 * from wherever one is: the cover large, every control, and the queue.
 * Closed, it gives back the page it was opened over, just as it was.
 */

import { useEffect } from "react";
import { Link } from "react-router-dom";
import { asClock } from "../../clock";
import { ChevronDownIcon, CloseIcon } from "../../icons";
import { useIsAFilmOnScreen } from "../../on-screen";
import { useSettings } from "../../settings";
import { Clock, Cover, Loudness, PlayPause, Progress, Ways } from "./bar";
import { NextIcon, PreviousIcon, StopIcon } from "./icons";
import { useMusic } from "./player";

export function MusicNowPlaying() {
  const { t } = useSettings();
  const music = useMusic();
  const film = useIsAFilmOnScreen();
  const { song, open, setOpen } = music;
  const shown = open && song !== null && !film;

  useEffect(() => {
    if (!shown) {
      return;
    }
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setOpen(false);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [shown, setOpen]);

  if (!shown || !song) {
    return null;
  }
  const close = () => setOpen(false);
  const { queue } = music;

  return (
    <div className="music-now" role="dialog" aria-modal="true" aria-label={t("music.now_playing")}>
      <div className="music-now-glow" aria-hidden="true" />
      <button type="button" className="music-control music-now-close" onClick={close} aria-label={t("music.close_player")} title={t("music.close_player")}>
        <ChevronDownIcon size={26} />
      </button>

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
          <Progress music={music} standing />
          <div className="music-now-clock">
            <Clock />
          </div>
          <div className="music-now-controls">
            <Ways music={music} />
            <button type="button" className="music-control" onClick={music.previous} aria-label={t("music.previous")} title={t("music.previous")}>
              <PreviousIcon size={26} />
            </button>
            <PlayPause music={music} large />
            <button type="button" className="music-control" onClick={music.next} aria-label={t("music.next")} title={t("music.next")}>
              <NextIcon size={26} />
            </button>
            <button type="button" className="music-control" onClick={music.stop} aria-label={t("music.stop")} title={t("music.stop")}>
              <StopIcon size={22} />
            </button>
          </div>
          <Loudness music={music} />
        </section>

        <section className="music-now-queue" aria-label={t("music.queue")}>
          <h3>{t("music.queue")}</h3>
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
                    <button type="button" className="music-control music-queue-remove" onClick={() => music.remove(at)} aria-label={t("music.take_out", { title: one.title })} title={t("music.take_out", { title: one.title })}>
                      <CloseIcon size={14} />
                    </button>
                  )}
                </li>
              );
            })}
          </ol>
        </section>
      </div>
    </div>
  );
}
