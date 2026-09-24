/*
 * A trailer hosted on YouTube, played behind this interface's own controls.
 *
 * The server never fetches the video: the browser asks YouTube for it through
 * the player YouTube offers for other sites to embed, told to draw none of its
 * own controls, and every button the viewer presses is this interface's,
 * driving that player through the programming interface it publishes for
 * exactly this. What reaches the screen is the picture; how it is driven is
 * Melyxar's.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { Transport } from "./engine";
import { useKeptLoudness } from "./loudness";

/** Which video a link to YouTube names, or nothing for any other link. */
export function youTubeKey(link: string): string | null {
  let address: URL;
  try {
    address = new URL(link);
  } catch {
    return null;
  }
  const host = address.hostname.replace(/^www\.|^m\./, "");
  if (host === "youtube.com" && address.pathname === "/watch") {
    return address.searchParams.get("v") || null;
  }
  if (host === "youtu.be") {
    return address.pathname.slice(1) || null;
  }
  return null;
}

/** The little of YouTube's player this uses, as it publishes it. */
interface YouTubePlayer {
  playVideo(): void;
  pauseVideo(): void;
  seekTo(seconds: number, allowSeekAhead: boolean): void;
  getCurrentTime(): number;
  getDuration(): number;
  getVideoLoadedFraction(): number;
  getPlayerState(): number;
  setVolume(volume: number): void;
  mute(): void;
  unMute(): void;
  destroy(): void;
}

interface YouTubeApi {
  Player: new (
    element: HTMLElement,
    options: {
      host: string;
      videoId: string;
      width: string;
      height: string;
      playerVars: Record<string, string | number>;
      events: {
        onReady: () => void;
        onStateChange: (event: { data: number }) => void;
        onError: () => void;
      };
    },
  ) => YouTubePlayer;
}

declare global {
  interface Window {
    YT?: YouTubeApi;
    onYouTubeIframeAPIReady?: () => void;
  }
}

/** YouTube's word for a video that is playing. */
const PLAYING = 1;

/** How often the player is asked where it is: often enough for the bar to
 *  move smoothly, rarely enough to cost nothing. */
const ASKED_EVERY_MS = 250;

let loading: Promise<YouTubeApi> | null = null;

/** YouTube's programming interface, fetched once for the whole page. */
function youTubeApi(): Promise<YouTubeApi> {
  if (window.YT?.Player) {
    return Promise.resolve(window.YT);
  }
  loading ??= new Promise<YouTubeApi>((resolve, reject) => {
    const before = window.onYouTubeIframeAPIReady;
    window.onYouTubeIframeAPIReady = () => {
      before?.();
      if (window.YT) {
        resolve(window.YT);
      }
    };
    const script = document.createElement("script");
    script.src = "https://www.youtube.com/iframe_api";
    script.async = true;
    script.onerror = () => {
      // Asked again next time rather than remembered as broken: a network
      // that was down a moment ago may be up now.
      loading = null;
      reject(new Error("YouTube could not be reached"));
    };
    document.head.appendChild(script);
  });
  return loading;
}

/**
 * Plays one YouTube video in the element handed over, and drives it.
 *
 * The element is YouTube's once the player is in it: nothing drawn by React
 * goes inside, and it is emptied again on the way out.
 */
export function useYouTubeTrailer(
  holder: React.RefObject<HTMLDivElement | null>,
  videoKey: string,
): { transport: Transport; failed: boolean } {
  const player = useRef<YouTubePlayer | null>(null);
  const [ready, setReady] = useState(false);
  const [failed, setFailed] = useState(false);
  const [at, setAt] = useState(0);
  const [length, setLength] = useState(0);
  const [loaded, setLoaded] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [sound, hear] = useKeptLoudness();

  useEffect(() => {
    const box = holder.current;
    if (!box) {
      return;
    }
    let gone = false;
    const inside = document.createElement("div");
    box.appendChild(inside);

    youTubeApi()
      .then((api) => {
        if (gone) {
          return;
        }
        player.current = new api.Player(inside, {
          // The address that sets no cookie on the viewer until they play.
          host: "https://www.youtube-nocookie.com",
          videoId: videoKey,
          width: "100%",
          height: "100%",
          playerVars: {
            autoplay: 1,
            controls: 0,
            disablekb: 1,
            fs: 0,
            iv_load_policy: 3,
            playsinline: 1,
            rel: 0,
            origin: window.location.origin,
          },
          events: {
            onReady: () => setReady(true),
            onStateChange: (event) => setPlaying(event.data === PLAYING),
            onError: () => setFailed(true),
          },
        });
      })
      .catch(() => {
        if (!gone) {
          setFailed(true);
        }
      });

    return () => {
      gone = true;
      player.current?.destroy();
      player.current = null;
      box.replaceChildren();
    };
  }, [holder, videoKey]);

  /* The sound the viewer keeps for every film, applied once the player can
     hear it and again whenever it changes. */
  useEffect(() => {
    const it = player.current;
    if (!ready || !it) {
      return;
    }
    it.setVolume(Math.round(sound.volume * 100));
    if (sound.muted) {
      it.mute();
    } else {
      it.unMute();
    }
  }, [ready, sound]);

  useEffect(() => {
    if (!ready) {
      return;
    }
    const ask = () => {
      const it = player.current;
      if (!it) {
        return;
      }
      const whole = it.getDuration();
      setAt(it.getCurrentTime());
      setLength(whole);
      setLoaded(it.getVideoLoadedFraction() * whole);
      setPlaying(it.getPlayerState() === PLAYING);
    };
    ask();
    const every = window.setInterval(ask, ASKED_EVERY_MS);
    return () => window.clearInterval(every);
  }, [ready]);

  const goTo = useCallback(
    (seconds: number) => {
      const place = Math.min(Math.max(0, seconds), length || seconds);
      player.current?.seekTo(place, true);
      setAt(place);
    },
    [length],
  );

  const transport = useMemo<Transport>(
    () => ({
      at,
      length,
      loaded,
      playing,
      muted: sound.muted,
      loudness: sound.volume,
      setLoudness: (volume) => hear({ volume, muted: false }),
      setMuted: (muted) => hear({ muted }),
      playOrPause: () => {
        const it = player.current;
        if (it) {
          if (playing) {
            it.pauseVideo();
          } else {
            it.playVideo();
          }
        }
      },
      stepBy: (seconds) => goTo(at + seconds),
      goTo,
    }),
    [at, length, loaded, playing, sound, hear, goTo],
  );

  return { transport, failed };
}
