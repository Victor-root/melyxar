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

/** What a trailer is doing, for the stage it plays on. */
export interface TrailerState {
  transport: Transport;
  failed: boolean;
  /** Still on its way, before anything can be pressed. */
  loading: boolean;
  /** Whether the picture may be shown right now. */
  shown: boolean;
  /** Played to its end. */
  ended: boolean;
}
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

/** YouTube's words for where its player stands. */
const ENDED = 0;
const PLAYING = 1;
const BUFFERING = 3;

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
 * How long YouTube's player keeps its own controls over the picture once it
 * has started playing, before they fade by themselves. Measured on the
 * maintainer's screen as a few seconds; a little more is kept to be sure.
 */
const ITS_CONTROLS_FADE_MS = 3_500;

/** A moment close enough to another not to be worth a jump. */
const CLOSE_ENOUGH = 0.5;

/**
 * Plays one YouTube video in the element handed over, and drives it.
 *
 * The element is YouTube's once the player is in it: nothing drawn by React
 * goes inside, and it is emptied again on the way out.
 *
 * Whether the picture may be shown is part of the answer. YouTube's player
 * draws its own things over the picture whenever it is not playing: a play
 * button before it starts, its title and suggestions when paused, a wall of
 * other videos at the end. And for a few seconds after it starts playing, its
 * own controls, whatever it was told. So every start is a warm up: the video
 * plays silent behind the stage for as long as those controls stay, from a
 * little before the moment wanted, and is shown and heard only once it has
 * reached it with the controls gone. Nobody ever hears it without seeing it,
 * and nobody sees what YouTube draws.
 */
export function useYouTubeTrailer(
  holder: React.RefObject<HTMLDivElement | null>,
  videoKey: string,
): TrailerState {
  const player = useRef<YouTubePlayer | null>(null);
  const [ready, setReady] = useState(false);
  const [failed, setFailed] = useState(false);
  const [at, setAt] = useState(0);
  const [length, setLength] = useState(0);
  const [loaded, setLoaded] = useState(0);
  const [state, setState] = useState<number | null>(null);
  /* Where a warm up is leading to, while one is under way. The first one
     leads to the very beginning. */
  const [warmingTo, setWarmingTo] = useState<number | null>(0);
  const [started, setStarted] = useState(false);
  const [sound, hear] = useKeptLoudness();
  const playing = state === PLAYING;
  const warming = warmingTo !== null;

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
            // Silent from the first frame: the first warm up starts at once.
            mute: 1,
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
            onStateChange: (event) => setState(event.data),
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

  /* Silent while warming up; otherwise the sound the viewer keeps for every
     film, applied again whenever it changes. */
  useEffect(() => {
    const it = player.current;
    if (!ready || !it) {
      return;
    }
    if (warming) {
      it.mute();
      return;
    }
    it.setVolume(Math.round(sound.volume * 100));
    if (sound.muted) {
      it.mute();
    } else {
      it.unMute();
    }
  }, [ready, sound, warming]);

  /* The warm up ends once it has played long enough for YouTube's controls
     to have gone: put on the moment wanted if it is not already there, and
     shown. Counted from when it really plays, and again from nothing if it
     stops on the way. */
  useEffect(() => {
    if (warmingTo === null || !playing) {
      return;
    }
    const done = window.setTimeout(() => {
      const it = player.current;
      if (it && Math.abs(it.getCurrentTime() - warmingTo) > CLOSE_ENOUGH) {
        it.seekTo(warmingTo, true);
      }
      setWarmingTo(null);
      setStarted(true);
    }, ITS_CONTROLS_FADE_MS);
    return () => window.clearTimeout(done);
  }, [warmingTo, playing]);

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
      setState(it.getPlayerState());
    };
    ask();
    const every = window.setInterval(ask, ASKED_EVERY_MS);
    return () => window.clearInterval(every);
  }, [ready]);

  /* Put somewhere during a warm up, the warm up leads there instead. */
  const goTo = useCallback(
    (seconds: number) => {
      const place = Math.min(Math.max(0, seconds), length || seconds);
      if (warming) {
        player.current?.seekTo(Math.max(0, place - ITS_CONTROLS_FADE_MS / 1000), true);
        setWarmingTo(place);
        return;
      }
      player.current?.seekTo(place, true);
      setAt(place);
    },
    [length, warming],
  );

  const transport = useMemo<Transport>(
    () => ({
      /* The moment the viewer is waiting for, not the one the silent run
         has got to. */
      at: warmingTo ?? at,
      length,
      loaded,
      playing,
      muted: sound.muted,
      loudness: sound.volume,
      setLoudness: (volume) => hear({ volume, muted: false }),
      setMuted: (muted) => hear({ muted }),
      playOrPause: () => {
        const it = player.current;
        if (!it) {
          return;
        }
        if (playing) {
          // Stopped during a warm up, it waits where the viewer left it.
          if (warmingTo !== null) {
            it.seekTo(warmingTo, true);
            setWarmingTo(null);
          }
          it.pauseVideo();
          return;
        }
        // Every start is a warm up, leading back to where it stopped.
        const wanted = warmingTo ?? it.getCurrentTime();
        it.seekTo(Math.max(0, wanted - ITS_CONTROLS_FADE_MS / 1000), true);
        setWarmingTo(wanted);
        it.playVideo();
      },
      stepBy: (seconds) => goTo((warmingTo ?? at) + seconds),
      goTo,
    }),
    [at, warmingTo, length, loaded, playing, sound, hear, goTo],
  );

  return {
    transport,
    failed,
    loading: !failed && (!ready || (warming && (playing || state === BUFFERING))),
    shown: !warming && (playing || (started && state === BUFFERING)),
    ended: state === ENDED,
  };
}
