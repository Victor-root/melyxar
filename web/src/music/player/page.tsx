/*
 * The page of what is playing, opened over the page one is on from the bar:
 * the cover large and beside it the queue or the words of the song, between
 * the header and the bar, which stay where they are and are the same ones
 * whether the page is open or not. On a phone the page takes the whole
 * screen, controls included.
 * Closed, it gives back the page it was opened over, just as it was.
 */

import { useEffect, useLayoutEffect, useRef, useState } from "react";
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
import { setNowPlayingCovers } from "./covering";
import { useLeaving } from "./leaving";
import { useNowPlayingPage } from "./opening";
import { useMusic } from "./player";
import { QueuePanel } from "./queue-panel";
import { BehindThePlayer } from "./spectrum";

/** What the three pages of a phone are called, from the left: the queue, what
 *  is playing and the words. */
const PHONE_PAGES = ["music.queue", "music.now_playing", "music.lyrics"] as const;

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
  /* Which of the three pages of a phone is open, kept for as long as the page
     is, so that it opens again on the one it was left on. It begins on what
     is playing. */
  const [phonePage, setPhonePage] = useState(1);
  const page = useRef<HTMLDivElement>(null);
  const bottom = useRef<HTMLDivElement>(null);
  const onAPhone = useMediaQuery("(max-width: 760px)");
  const reduced = useMediaQuery("(prefers-reduced-motion: reduce)");
  const leaving = useLeaving(shown, song !== null && !film, reduced ? 0 : LEAVE_MS);
  /* On a phone the page covers the whole screen once it has faded in, and
     until it starts to leave. */
  const [arrived, setArrived] = useState(false);
  const covers = onAPhone && shown && (arrived || reduced);
  useEffect(() => {
    if (!shown) {
      setArrived(false);
    }
  }, [shown]);
  useEffect(() => {
    setNowPlayingCovers(covers);
    return () => setNowPlayingCovers(false);
  }, [covers]);

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
    <div
      ref={page}
      className={`music-now${leaving ? " music-now-leaving" : ""}`}
      role="dialog"
      aria-modal="true"
      aria-label={t("music.now_playing")}
      onAnimationEnd={(event) => {
        if (event.target === event.currentTarget && event.animationName === "music-now-fade") {
          setArrived(true);
        }
      }}
    >
      <PageBackdrop inPlace />
      {onAPhone && <PhoneHeader title={t(PHONE_PAGES[phonePage])} close={close} />}
      {onAPhone && <PageDots page={phonePage} />}

      {onAPhone ? (
        <PhonePages song={song} page={phonePage} onPage={setPhonePage} />
      ) : (
        <div className="music-now-body">
          <NowPlayingSong song={song} />

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
function NowPlayingSong({ song }: { song: Song }) {
  return (
    <section className="music-now-playing">
      <Cover pictures={song.cover} large />
      <div className="music-now-words">
        <h2>{song.title}</h2>
        <p className="music-now-by">
          {song.artists.map((artist, index) => (
            <span key={artist.id}>
              {index > 0 && ", "}
              <Link to={`/music/artist/${artist.id}`} replace>
                {artist.name}
              </Link>
            </span>
          ))}
        </p>
        {song.album && (
          <p className="music-now-album">
            <Link to={`/music/album/${song.album.id}`} replace>
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
        <Link className="brand" to="/" replace>
          <ServerMark branding={branding} size={28} logoClassName="brand-logo" />
        </Link>
      </div>
      <span className="music-now-heading">{title}</span>
    </div>
  );
}

/** Three dots at the top of the page, the one lit being the page open: where it
 *  stands among the others says there is more to the left or to the right. */
function PageDots({ page }: { page: number }) {
  return (
    <div className="music-now-dots" aria-hidden="true">
      {PHONE_PAGES.map((one, at) => (
        <span key={one} data-on={at === page ? "yes" : "no"} />
      ))}
    </div>
  );
}

/**
 * On a phone, three pages side by side and swiped between: the queue at the
 * left, what is playing in the middle with the cover, the words at the right.
 * The one that was open last is the one that opens.
 */
function PhonePages({
  song,
  page,
  onPage,
}: {
  song: Song;
  page: number;
  onPage: (page: number) => void;
}) {
  const pager = useRef<HTMLDivElement>(null);
  const queue = useRef<HTMLDivElement>(null);
  const pages = useRef<(HTMLDivElement | null)[]>([]);

  /* Opened on the page it was left on, before anything is drawn. */
  useLayoutEffect(() => {
    const element = pager.current;
    if (element) {
      element.scrollLeft = page * element.clientWidth;
    }
    // Only when it opens: afterwards the hand decides.
  }, []);

  useEffect(() => {
    const element = pager.current;
    if (!element) {
      return;
    }
    const watcher = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            onPage(pages.current.indexOf(entry.target as HTMLDivElement));
          }
        }
      },
      { root: element, threshold: 0.6 },
    );
    for (const one of pages.current) {
      if (one) {
        watcher.observe(one);
      }
    }
    return () => watcher.disconnect();
  }, [onPage]);

  const hold = (at: number) => (element: HTMLDivElement | null) => {
    pages.current[at] = element;
  };

  return (
    <>
      <div className="music-now-pages" ref={pager}>
        <div
          className="music-now-page"
          ref={(element) => {
            queue.current = element;
            hold(0)(element);
          }}
        >
          <section className="music-now-queue">
            <QueuePanel />
          </section>
        </div>
        <div className="music-now-page music-now-page-song" ref={hold(1)}>
          <NowPlayingSong song={song} />
        </div>
        <div className="music-now-page music-now-page-words" ref={hold(2)}>
          <section className="music-now-queue">
            <LyricsPanel song={song.id} />
          </section>
        </div>
      </div>
      <ScrollBar holder={queue} />
    </>
  );
}
