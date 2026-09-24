/*
 * A trailer, in this interface's own player.
 *
 * A trailer has no position to come back to, no soundtrack to choose, no
 * subtitles and no session: it is two minutes long and watched once. So it is
 * not the film player, whose machinery would stand idle here. But it wears the
 * same controls, driven the same way: the same bar, the same sound, the same
 * keys, the same way out and the film's own mark in the corner.
 *
 * Two kinds of trailer play here. One sitting next to the film is a file the
 * browser opens itself. One hosted on YouTube plays through the player YouTube
 * offers other sites, told to draw nothing of its own, with these controls in
 * front of it: nothing of it is fetched by the server.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { Work } from "../api";
import { useSettings } from "../settings";
import { asClock } from "./clock";
import { A_STEP } from "./engine";
import type { Transport } from "./engine";
import { useFullscreen } from "./fullscreen";
import {
  BackIcon,
  FullscreenIcon,
  PauseIcon,
  PlayIcon,
  StepBackIcon,
  StepOnIcon,
} from "./icons";
import { markFor, useBranding } from "./logo";
import { useKeptLoudness } from "./loudness";
import { Rail, Sound, useControlsFade, useTransportKeys } from "./overlay";
import { useYouTubeTrailer } from "./youtube";
import "./player.css";

/** Where a trailer plays from. */
export type TrailerSource = { file: string } | { youtube: string };

/* The sizes the film player draws its controls at. */
const ICON = 27;
const PLAY_ICON = 34;

export function TrailerPlayer({
  source,
  work,
  onClose,
}: {
  source: TrailerSource;
  work: Work;
  onClose: () => void;
}) {
  return "file" in source ? (
    <FileTrailer url={source.file} work={work} onClose={onClose} />
  ) : (
    <YouTubeTrailer videoKey={source.youtube} work={work} onClose={onClose} />
  );
}

function FileTrailer({ url, work, onClose }: { url: string; work: Work; onClose: () => void }) {
  const video = useRef<HTMLVideoElement>(null);
  const { transport, failed } = useFileTrailer(video);
  return (
    <TrailerStage work={work} transport={transport} failed={failed} onClose={onClose}>
      <video ref={video} className="player-video" src={url} autoPlay />
    </TrailerStage>
  );
}

function YouTubeTrailer({
  videoKey,
  work,
  onClose,
}: {
  videoKey: string;
  work: Work;
  onClose: () => void;
}) {
  const holder = useRef<HTMLDivElement>(null);
  const { transport, failed } = useYouTubeTrailer(holder, videoKey);
  return (
    <TrailerStage
      work={work}
      transport={transport}
      failed={failed}
      /* Some trailers are only allowed to play on YouTube itself: the way
         there is offered rather than a dead end. */
      elsewhere={`https://www.youtube.com/watch?v=${encodeURIComponent(videoKey)}`}
      onClose={onClose}
    >
      <div ref={holder} className="trailer-frame" />
    </TrailerStage>
  );
}

/**
 * The picture, and the controls over it.
 *
 * A layer lies over the picture and takes every click, so YouTube's player
 * underneath never sees a pointer and never draws anything of its own; a
 * click on it starts or stops the trailer, and two fill the screen.
 */
function TrailerStage({
  work,
  transport,
  failed,
  elsewhere,
  onClose,
  children,
}: {
  work: Work;
  transport: Transport;
  failed: boolean;
  /** Where it can be watched instead, when it will not play here. */
  elsewhere?: string;
  onClose: () => void;
  children: React.ReactNode;
}) {
  const { t } = useSettings();
  const stage = useRef<HTMLDivElement>(null);
  const fullscreen = useFullscreen(stage);
  const branding = useBranding();
  const mark = markFor(work.title, work.logo, branding);
  const { away, stir } = useControlsFade(stage, !transport.playing, work.id);
  const escape = useCallback(() => {
    if (document.fullscreenElement) {
      void document.exitFullscreen();
    } else {
      onClose();
    }
  }, [onClose]);
  useTransportKeys(transport, fullscreen, escape, stir);

  return (
    <div className="player" role="dialog" aria-label={`${work.title}, ${t("work.trailer")}`}>
      <div className="player-stage" ref={stage}>
        {/* A picture that will not play is taken away rather than left
            saying so in somebody else's words. */}
        {!failed && children}
        <div
          className="trailer-surface"
          onClick={transport.playOrPause}
          onDoubleClick={fullscreen.toggle}
        />

        {failed && (
          <div className="player-notices">
            <p className="player-notice">
              {t(elsewhere ? "player.trailer_not_here" : "player.cannot_play")}
            </p>
            {elsewhere && (
              <a
                className="button trailer-elsewhere"
                href={elsewhere}
                target="_blank"
                rel="noreferrer noopener"
              >
                {t("player.watch_on_youtube")}
              </a>
            )}
          </div>
        )}

        <div
          className="player-overlay"
          data-away={away ? "yes" : "no"}
          onPointerDown={(event) => event.stopPropagation()}
        >
          <div className="player-top">
            <div className="player-zone player-zone-top-left">
              <button className="player-button" onClick={onClose} aria-label={t("player.close")}>
                <BackIcon size={ICON} />
              </button>
              {mark.url ? (
                <img
                  className={`player-mark player-mark-${mark.whose.replace("_", "-")}`}
                  src={mark.url}
                  srcSet={mark.srcSet ?? undefined}
                  sizes="340px"
                  alt={mark.words}
                />
              ) : (
                <span className="player-title">{mark.words}</span>
              )}
              <span className="player-trailer-word">{t("work.trailer")}</span>
            </div>
          </div>

          <div className="player-bottom">
            <div className="player-seek">
              <span className="player-seek-end">
                <span className="player-clock">{asClock(transport.at)}</span>
              </span>
              <Rail
                transport={transport}
                thumbnails={null}
                previewScale={1}
                label={t("player.position")}
              />
              <span className="player-seek-end">
                <span className="player-clock">
                  {transport.length > 0 ? `-${asClock(transport.length - transport.at)}` : ""}
                </span>
              </span>
            </div>
            <div className="player-row">
              <div className="player-zone player-zone-bottom-left">
                <button
                  className="player-button"
                  onClick={() => transport.stepBy(-A_STEP)}
                  aria-label={t("player.back_ten")}
                >
                  <StepBackIcon seconds={A_STEP} size={ICON} />
                </button>
                <button
                  className="player-button player-button-play"
                  onClick={transport.playOrPause}
                  aria-label={t(transport.playing ? "player.pause" : "player.play")}
                >
                  {transport.playing ? <PauseIcon size={PLAY_ICON} /> : <PlayIcon size={PLAY_ICON} />}
                </button>
                <button
                  className="player-button"
                  onClick={() => transport.stepBy(A_STEP)}
                  aria-label={t("player.on_ten")}
                >
                  <StepOnIcon seconds={A_STEP} size={ICON} />
                </button>
                <Sound transport={transport} t={t} />
              </div>
              <div className="player-zone player-zone-bottom-right">
                <button
                  className="player-button"
                  onClick={fullscreen.toggle}
                  aria-label={t(fullscreen.filling ? "player.leave_fullscreen" : "player.fullscreen")}
                >
                  <FullscreenIcon leaving={fullscreen.filling} size={ICON} />
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

/**
 * Drives a trailer the browser plays itself, read off its element.
 *
 * A trailer the browser cannot open says so rather than showing a black
 * rectangle. There is no conversion behind this: a file the server put there
 * itself is almost always one a browser opens, and a whole engine for the
 * exception would cost more than it saves.
 */
function useFileTrailer(video: React.RefObject<HTMLVideoElement | null>): {
  transport: Transport;
  failed: boolean;
} {
  const [failed, setFailed] = useState(false);
  const [at, setAt] = useState(0);
  const [length, setLength] = useState(0);
  const [loaded, setLoaded] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [sound, hear] = useKeptLoudness();

  useEffect(() => {
    const element = video.current;
    if (!element) {
      return;
    }
    element.volume = sound.volume;
    element.muted = sound.muted;
  }, [video, sound]);

  useEffect(() => {
    const element = video.current;
    if (!element) {
      return;
    }
    const read = () => {
      setAt(element.currentTime);
      setLength(Number.isFinite(element.duration) ? element.duration : 0);
      setLoaded(element.buffered.length > 0 ? element.buffered.end(element.buffered.length - 1) : 0);
      setPlaying(!element.paused && !element.ended);
    };
    const broke = () => setFailed(true);
    const heard = ["timeupdate", "durationchange", "progress", "play", "pause", "ended"];
    for (const name of heard) {
      element.addEventListener(name, read);
    }
    element.addEventListener("error", broke);
    return () => {
      for (const name of heard) {
        element.removeEventListener(name, read);
      }
      element.removeEventListener("error", broke);
    };
  }, [video]);


  const transport = useMemo<Transport>(() => {
    const goTo = (seconds: number) => {
      const element = video.current;
      if (element) {
        element.currentTime = Math.min(Math.max(0, seconds), element.duration || seconds);
      }
    };
    return {
      at,
      length,
      loaded,
      playing,
      muted: sound.muted,
      loudness: sound.volume,
      setLoudness: (volume) => hear({ volume, muted: false }),
      setMuted: (muted) => hear({ muted }),
      playOrPause: () => {
        const element = video.current;
        if (element) {
          if (element.paused) {
            void element.play();
          } else {
            element.pause();
          }
        }
      },
      stepBy: (seconds) => goTo(at + seconds),
      goTo,
    };
  }, [video, at, length, loaded, playing, sound, hear]);

  return { transport, failed };
}
