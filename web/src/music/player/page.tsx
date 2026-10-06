/*
 * The page of what is playing, opened over the page one is on from the bar:
 * the cover large and beside it the queue or the words of the song, between
 * the header and the bar, which stay where they are and are the same ones
 * whether the page is open or not. On a phone the page takes the whole
 * screen, controls included.
 * Closed, it gives back the page it was opened over, just as it was.
 */

import { useEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import type { Song } from "../api";
import { PageBackdrop } from "../../components/backdrop";
import { ScrollBar } from "../../components/scrollbar";
import { ServerMark } from "../../components/server-mark";
import { BackIcon } from "../../icons";
import { useMediaQuery } from "../../media-query";
import { useIsAFilmOnScreen } from "../../on-screen";
import { useBranding } from "../../player/logo";
import { useSettings } from "../../settings";
import { Cover } from "./bar";
import { HeartButton, Rail, StopButton, Transport, Volume, Ways } from "./controls";
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
  const [onLyrics, setOnLyrics] = useState(false);
  const page = useRef<HTMLDivElement>(null);
  const bottom = useRef<HTMLDivElement>(null);
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

  /* The pages run behind the foot of the screen, which is clear glass over
     them: how tall it is says how much room they keep at their end. */
  const hasBottom = onAPhone && song !== null;
  useEffect(() => {
    const element = bottom.current;
    const root = page.current;
    if (!hasBottom || !element || !root) {
      return;
    }
    const watcher = new ResizeObserver(() => root.style.setProperty("--now-bottom", `${element.offsetHeight}px`));
    watcher.observe(element);
    return () => watcher.disconnect();
  }, [hasBottom, shown, leaving]);

  if ((!shown && !leaving) || !song) {
    return null;
  }

  return (
    <div ref={page} className={`music-now music-dark${leaving ? " music-now-leaving" : ""}`} role="dialog" aria-modal="true" aria-label={t("music.now_playing")}>
      <PageBackdrop inPlace />
      {onAPhone && <PhoneHeader title={t(onLyrics ? "music.lyrics" : "music.queue")} close={close} />}

      {onAPhone ? (
        <PhonePages song={song} close={close} onLyrics={setOnLyrics} />
      ) : (
        <div className="music-now-body">
          <NowPlayingSong song={song} close={close} />

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
      )}

      {onAPhone && (
        <div className="player-bottom" ref={bottom}>
          <BehindThePlayer />
          <Rail music={music} />
          <div className="player-row">
            <div className="player-zone player-zone-bottom-left">
              <Transport music={music} withStop={false} greyedWhenNone disc />
            </div>
            <div className="player-zone player-zone-bottom-right">
              <HeartButton id={song.id} />
              <Volume music={music} />
              <Ways music={music} />
              <StopButton music={music} />
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

/** The cover large, then the name of the song, who plays it and the album. */
function NowPlayingSong({ song, close }: { song: Song; close: () => void }) {
  return (
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
  );
}

/** The left piece of the bar at the top, the one every page has, and the name
 *  of what is shown beside it. */
function PhoneHeader({ title, close }: { title: string; close: () => void }) {
  const { t } = useSettings();
  const branding = useBranding();
  return (
    <div className="header-inner music-now-header">
      <div className="header-piece header-start header-start-back">
        <button type="button" className="header-icon" onClick={close} title={t("music.close_player")} aria-label={t("music.close_player")}>
          <BackIcon size={22} />
        </button>
        <Link className="brand" to="/" onClick={close}>
          <ServerMark branding={branding} size={28} logoClassName="brand-logo" />
        </Link>
      </div>
      <span className="music-now-heading">{title}</span>
    </div>
  );
}

/**
 * On a phone, the queue under the cover and the words of the song are two
 * pages side by side: a swipe to the right leaves only the words, a swipe
 * back brings the cover and the queue again.
 */
function PhonePages({
  song,
  close,
  onLyrics,
}: {
  song: Song;
  close: () => void;
  onLyrics: (shown: boolean) => void;
}) {
  const pager = useRef<HTMLDivElement>(null);
  const queue = useRef<HTMLDivElement>(null);
  const lyrics = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const element = pager.current;
    if (!element || !lyrics.current) {
      return;
    }
    const watcher = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.target === lyrics.current && entry.isIntersecting) {
            onLyrics(true);
          } else if (entry.target === queue.current && entry.isIntersecting) {
            onLyrics(false);
          }
        }
      },
      { root: element, threshold: 0.6 },
    );
    if (queue.current) {
      watcher.observe(queue.current);
    }
    watcher.observe(lyrics.current);
    return () => watcher.disconnect();
  }, [onLyrics]);

  return (
    <>
      <div className="music-now-pages" ref={pager}>
        <div className="music-now-page music-now-page-queue" ref={queue}>
          <NowPlayingSong song={song} close={close} />
          <section className="music-now-queue">
            <QueuePanel />
          </section>
        </div>
        <div className="music-now-page" ref={lyrics}>
          <section className="music-now-queue">
            <LyricsPanel song={song.id} />
          </section>
        </div>
      </div>
      <ScrollBar holder={queue} />
    </>
  );
}
