/*
 * The playback bar, drawn here rather than by the browser.
 *
 * A browser draws a perfectly good bar of its own, and it was the right answer
 * for as long as nothing more was asked of it. Showing the picture of the
 * moment under the cursor is more: it means knowing where the cursor is on the
 * bar and putting something there, and the browser's bar says neither. So this
 * one is ours, and everything the browser's did has to be here too.
 *
 * The little pictures come as sheets of a hundred. The page fetches the sheet
 * holding the moment under the cursor and cuts the thumbnail out of it with a
 * background offset, which is one request for a thousand seconds of film
 * instead of one per picture.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import type { PlaybackThumbnails } from "../api";
import { rememberLoudness, storedLoudness } from "./loudness";

/**
 * Starts a film, or stops it.
 *
 * Here rather than in the button alone because the picture itself answers to a
 * click too, and two places deciding what a click means is two places for them
 * to disagree.
 */
export function playOrPause(element: HTMLVideoElement | null) {
  if (!element) {
    return;
  }
  if (element.paused) {
    void element.play();
  } else {
    element.pause();
  }
}

interface Props {
  /* The element being driven. Held by the player, which mounts a fresh one
     whenever the picture changes, so the key below says when to listen to
     another one. */
  video: React.RefObject<HTMLVideoElement | null>;
  /* Changes when the element does. */
  pictureKey: string | null;
  /* What goes fullscreen: the bar has to come with the picture, and a video
     element sent fullscreen on its own leaves it behind. */
  stage: React.RefObject<HTMLDivElement | null>;
  thumbnails: PlaybackThumbnails | null;
  /* Told when the viewer starts moving the film and again when they have
     finished, never in between: dragging along the bar moves the film at every
     twitch, and what happens on each of those is work thrown away.
     What the two do is the player's business. */
  onViewerMoving: () => void;
  onViewerMoved: () => void;
  t: (key: string, values?: Record<string, string | number>) => string;
}

/**
 * How far one step goes, in seconds.
 *
 * Ten, which is what every player uses and what the thing is for: missing a
 * line of dialogue, not choosing a scene. The bar is there for choosing a
 * scene.
 */
const A_STEP: number = 10;

/** A moment of a film, as somebody reads it. */
function asClock(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) {
    return "0:00";
  }
  const whole = Math.floor(seconds);
  const hours = Math.floor(whole / 3600);
  const minutes = Math.floor((whole % 3600) / 60);
  const rest = whole % 60;
  const padded = `${minutes < 10 && hours > 0 ? "0" : ""}${minutes}:${rest < 10 ? "0" : ""}${rest}`;
  return hours > 0 ? `${hours}:${padded}` : padded;
}

/** Which thumbnail covers a moment, and where it sits on its sheet. */
function spotOf(
  thumbnails: PlaybackThumbnails,
  seconds: number,
): { sheet: number; column: number; row: number } | null {
  const perSheet = thumbnails.columns * thumbnails.rows;
  if (thumbnails.every_seconds <= 0 || perSheet <= 0 || thumbnails.counted <= 0) {
    return null;
  }
  const index = Math.floor(Math.max(0, seconds) / thumbnails.every_seconds);
  // Never past the last one: a sheet is filled to the end with black whatever
  // the film gave, and a black square under the cursor is worse than none.
  const kept = Math.min(index, thumbnails.counted - 1);
  const onItsSheet = kept % perSheet;
  return {
    sheet: Math.floor(kept / perSheet),
    column: onItsSheet % thumbnails.columns,
    row: Math.floor(onItsSheet / thumbnails.columns),
  };
}

export function Controls({
  video,
  pictureKey,
  stage,
  thumbnails,
  onViewerMoving,
  onViewerMoved,
  t,
}: Props) {
  const [playing, setPlaying] = useState(false);
  const [at, setAt] = useState(0);
  const [length, setLength] = useState(0);
  const [loaded, setLoaded] = useState(0);
  const [muted, setMuted] = useState(false);
  const [loudness, setLoudness] = useState(1);
  const [fullscreen, setFullscreen] = useState(false);
  /* Where the cursor is along the bar, from nought to one, while it is on it.
     Null the rest of the time, which is what hides the preview. */
  const [hovered, setHovered] = useState<number | null>(null);
  /* How wide the bar is, taken whenever the cursor is on it. The preview is
     placed in pixels rather than in hundredths, because keeping it on screen
     at both ends means knowing how wide it is against how wide the bar is. */
  const [railWidth, setRailWidth] = useState(0);
  const [dragging, setDragging] = useState(false);
  /* Whether the bar has faded out. It sits on top of the picture, so leaving
     it there for ever would mean watching a film with a strip of controls
     across the bottom of it. */
  const [idle, setIdle] = useState(false);
  const rail = useRef<HTMLDivElement>(null);

  /* Everything shown here is read off the element rather than remembered
     alongside it: the film is what moves, and a copy of where it has got to is
     a copy that goes wrong the moment anything else moves it. */
  useEffect(() => {
    const element = video.current;
    if (!element) {
      return;
    }
    /* The element is a new one for every film and starts at full volume, so
       the setting is put back on it before anything is heard. */
    const wanted = storedLoudness();
    element.volume = wanted.volume;
    element.muted = wanted.muted;
    const tell = () => {
      setAt(element.currentTime);
      setLength(Number.isFinite(element.duration) ? element.duration : 0);
      setPlaying(!element.paused && !element.ended);
      setMuted(element.muted);
      setLoudness(element.volume);
      const buffered = element.buffered;
      setLoaded(buffered.length > 0 ? buffered.end(buffered.length - 1) : 0);
    };
    tell();
    const events = [
      "timeupdate",
      "durationchange",
      "loadedmetadata",
      "play",
      "pause",
      "ended",
      "progress",
      "volumechange",
      "seeking",
      "seeked",
    ];
    for (const name of events) {
      element.addEventListener(name, tell);
    }
    /* Whatever moves the sound, wherever from: the buttons here, a keyboard
       key the browser answers on its own, a headset. */
    const remember = () =>
      rememberLoudness({ volume: element.volume, muted: element.muted });
    element.addEventListener("volumechange", remember);
    return () => {
      for (const name of events) {
        element.removeEventListener(name, tell);
      }
      element.removeEventListener("volumechange", remember);
    };
  }, [video, pictureKey]);

  useEffect(() => {
    const tell = () => setFullscreen(document.fullscreenElement !== null);
    tell();
    document.addEventListener("fullscreenchange", tell);
    return () => document.removeEventListener("fullscreenchange", tell);
  }, []);

  /* Where along the film a point on the bar is. */
  const shareAt = useCallback((clientX: number): number | null => {
    const bar = rail.current?.getBoundingClientRect();
    if (!bar || bar.width <= 0) {
      return null;
    }
    setRailWidth(bar.width);
    return Math.min(1, Math.max(0, (clientX - bar.left) / bar.width));
  }, []);

  /* Shown while anything is moving, while nothing is playing, and while the
     cursor is on the bar itself: a paused film is a film somebody is about to
     do something with, and a cursor resting on the bar moves no more than one
     resting anywhere else. */
  useEffect(() => {
    const stirred = stage.current;
    if (!stirred) {
      return;
    }
    let timer = 0;
    const wake = () => {
      setIdle(false);
      window.clearTimeout(timer);
      timer = window.setTimeout(() => setIdle(true), 2_500);
    };
    wake();
    stirred.addEventListener("pointermove", wake);
    stirred.addEventListener("pointerdown", wake);
    return () => {
      window.clearTimeout(timer);
      stirred.removeEventListener("pointermove", wake);
      stirred.removeEventListener("pointerdown", wake);
    };
  }, [stage, pictureKey]);

  const goTo = useCallback(
    (share: number) => {
      const element = video.current;
      if (element && length > 0) {
        element.currentTime = share * length;
        setAt(share * length);
      }
    },
    [video, length],
  );

  /* A step back or on, from the buttons and from the arrow keys alike: one
     way of moving means one place for it to be wrong. Held inside the film at
     both ends, because a step past the end is the film over. */
  const stepBy = useCallback(
    (seconds: number) => {
      const element = video.current;
      if (!element) {
        return;
      }
      const furthest = length > 0 ? length : element.duration;
      const wanted = element.currentTime + seconds;
      onViewerMoving();
      element.currentTime = Math.max(
        0,
        Number.isFinite(furthest) ? Math.min(furthest, wanted) : wanted,
      );
      onViewerMoved();
    },
    [video, length, onViewerMoving, onViewerMoved],
  );

  /* Dragging is followed on the window rather than on the bar: a finger that
     leaves the bar while still held down is still dragging, and a bar that
     stops following it there is a bar that jumps back. */
  useEffect(() => {
    if (!dragging) {
      return;
    }
    const moved = (event: PointerEvent) => {
      const share = shareAt(event.clientX);
      if (share !== null) {
        setHovered(share);
        goTo(share);
      }
    };
    const let_go = () => {
      setDragging(false);
      onViewerMoved();
    };
    window.addEventListener("pointermove", moved);
    window.addEventListener("pointerup", let_go);
    window.addEventListener("pointercancel", let_go);
    return () => {
      window.removeEventListener("pointermove", moved);
      window.removeEventListener("pointerup", let_go);
      window.removeEventListener("pointercancel", let_go);
    };
  }, [dragging, shareAt, goTo, onViewerMoved]);

  const playing_share = length > 0 ? Math.min(1, at / length) : 0;
  const loaded_share = length > 0 ? Math.min(1, loaded / length) : 0;
  const previewed = hovered !== null && length > 0 ? hovered * length : null;
  const spot = thumbnails && previewed !== null ? spotOf(thumbnails, previewed) : null;
  /* Kept on screen at both ends rather than half off it. */
  const half = (thumbnails?.width ?? 0) / 2;
  const previewLeft = Math.min(
    Math.max((hovered ?? 0) * railWidth, half),
    Math.max(half, railWidth - half),
  );

  return (
    <div
      className={`player-controls${
        idle && playing && hovered === null && !dragging ? " player-controls-away" : ""
      }`}
    >
      <div
        className="player-rail"
        ref={rail}
        role="slider"
        tabIndex={0}
        aria-label={t("player.position")}
        aria-valuemin={0}
        aria-valuemax={Math.round(length)}
        aria-valuenow={Math.round(at)}
        aria-valuetext={asClock(at)}
        onPointerDown={(event) => {
          const share = shareAt(event.clientX);
          if (share !== null) {
            onViewerMoving();
            setDragging(true);
            setHovered(share);
            goTo(share);
          }
        }}
        onPointerMove={(event) => setHovered(shareAt(event.clientX))}
        onPointerLeave={() => {
          if (!dragging) {
            setHovered(null);
          }
        }}
        onKeyDown={(event) => {
          if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
            event.preventDefault();
            stepBy(event.key === "ArrowLeft" ? -A_STEP : A_STEP);
          }
        }}
      >
        <span className="player-rail-loaded" style={{ width: `${loaded_share * 100}%` }} />
        <span className="player-rail-played" style={{ width: `${playing_share * 100}%` }} />
        <span className="player-rail-handle" style={{ left: `${playing_share * 100}%` }} />

        {/* The preview stands above the bar and follows the cursor, kept
            inside the bar at both ends so it is never half off the screen.
            Held by the bar rather than by the row of controls, so that what it
            clears is the bar itself whether it carries a picture or only the
            time: measured off the row, the time sat on the bar. */}
        {previewed !== null && (
          <div className="player-preview" style={{ left: `${previewLeft}px` }} aria-hidden="true">
            {spot && thumbnails && (
              <span
                className="player-preview-picture"
                style={{
                  width: `${thumbnails.width}px`,
                  height: `${thumbnails.height}px`,
                  backgroundImage: `url(${thumbnails.url}/${spot.sheet}.jpg)`,
                  backgroundPosition: `-${spot.column * thumbnails.width}px -${spot.row * thumbnails.height}px`,
                }}
              />
            )}
            <span className="player-preview-time">{asClock(previewed)}</span>
          </div>
        )}
      </div>

      <div className="player-buttons">
        {/* A step back and a step on, around the button that starts the film.
            For the line of dialogue somebody missed: choosing a scene is what
            the bar above is for, and a bar is no good at ten seconds. */}
        <button
          className="player-button player-button-step"
          onClick={() => stepBy(-A_STEP)}
          aria-label={t("player.back_ten")}
        >
          {`↺${A_STEP}`}
        </button>

        <button
          className="player-button"
          onClick={() => playOrPause(video.current)}
          aria-label={t(playing ? "player.pause" : "player.play")}
        >
          {playing ? "⏸" : "▶"}
        </button>

        <button
          className="player-button player-button-step"
          onClick={() => stepBy(A_STEP)}
          aria-label={t("player.on_ten")}
        >
          {`↻${A_STEP}`}
        </button>

        <span className="player-clock">
          {asClock(at)}
          <span className="player-clock-total"> / {asClock(length)}</span>
        </span>

        <span className="player-sound">
          <button
            className="player-button"
            onClick={() => {
              const element = video.current;
              if (element) {
                element.muted = !element.muted;
              }
            }}
            aria-label={t(muted ? "player.unmute" : "player.mute")}
          >
            {muted || loudness === 0 ? "\u{1F507}" : "\u{1F50A}"}
          </button>
          <input
            className="player-loudness"
            type="range"
            min={0}
            max={1}
            step={0.01}
            value={muted ? 0 : loudness}
            aria-label={t("player.loudness")}
            onChange={(event) => {
              const element = video.current;
              if (element) {
                element.volume = Number(event.target.value);
                element.muted = Number(event.target.value) === 0;
              }
            }}
          />
        </span>

        <button
          className="player-button"
          onClick={() => {
            if (document.fullscreenElement) {
              void document.exitFullscreen();
            } else {
              void stage.current?.requestFullscreen?.();
            }
          }}
          aria-label={t(fullscreen ? "player.leave_fullscreen" : "player.fullscreen")}
        >
          {fullscreen ? "⤡" : "⤢"}
        </button>
      </div>
    </div>
  );
}
