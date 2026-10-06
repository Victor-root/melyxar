/*
 * The controls of the player of music, drawn with the classes and the icons
 * of the player of films: the two are one design with two engines behind
 * it, and a button that looked different here would be a second player to
 * learn.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
import { asClock } from "../../clock";
import { HeartIcon } from "../../icons";
import {
  NextEpisodeIcon,
  PauseIcon,
  PlayIcon,
  PreviousEpisodeIcon,
  StepBackIcon,
  StepOnIcon,
} from "../../player/icons";
import { previewPlace } from "../../player/seek";
import { ICON, SoundControl } from "../../player/sound";
import { useSettings } from "../../settings";
import { useLiking } from "../marks";
import { QueueIcon, RepeatIcon, ShuffleIcon, StopIcon } from "./icons";
import { useNowPlayingPage } from "./opening";
import { useMusicTime } from "./player";
import type { Music } from "./player";
import { canStepBack, canStepOn } from "./queue";

/** How large the play button's icon is drawn. */
const PLAY_ICON = 34;

/** How far the arrow keys jump, in seconds. */
const KEY_STEP = 5;

/** How wide the time shown over the bar is, for keeping it inside it. */
const TIME_ACROSS = 64;

/** The play and pause button. As a disc it is the one a song's line has at its
 *  left while it is the one playing, only larger. */
export function PlayButton({ music, disc = false }: { music: Music; disc?: boolean }) {
  const { t } = useSettings();
  return (
    <button
      type="button"
      className={`${disc ? "music-song-toggle music-song-toggle-lit" : "player-button player-button-play"}${music.waiting ? " music-waiting" : ""}`}
      onClick={music.toggle}
      aria-label={t(music.playing ? "music.pause" : "music.play")}
    >
      {music.playing ? <PauseIcon size={disc ? 26 : PLAY_ICON} /> : <PlayIcon size={disc ? 26 : PLAY_ICON} />}
    </button>
  );
}

export function SongStepButton({ music, back, greyedWhenNone }: { music: Music; back: boolean; greyedWhenNone?: boolean }) {
  const { t } = useSettings();
  const greyed = greyedWhenNone === true && !(back ? canStepBack(music.queue) : canStepOn(music.queue));
  return (
    <button
      type="button"
      className="player-button"
      disabled={greyed}
      onClick={back ? music.previous : music.next}
      aria-label={t(back ? "music.previous" : "music.next")}
    >
      {back ? <PreviousEpisodeIcon size={ICON} /> : <NextEpisodeIcon size={ICON} />}
    </button>
  );
}

/** The two jumps within the song, back and on. */
export function SecondsButton({ music, back }: { music: Music; back: boolean }) {
  const { t } = useSettings();
  const { position, length } = useMusicTime();
  const step = back ? music.preferences.skip_back_seconds : music.preferences.skip_on_seconds;
  const go = () => music.seek(Math.min(Math.max(position + (back ? -step : step), 0), length));
  return (
    <button
      type="button"
      className="player-button"
      onClick={go}
      aria-label={t(back ? "player.step_back" : "player.step_on", { seconds: step })}
    >
      {back ? <StepBackIcon seconds={step} size={ICON} /> : <StepOnIcon seconds={step} size={ICON} />}
    </button>
  );
}

export function StopButton({ music }: { music: Music }) {
  const { t } = useSettings();
  return (
    <button type="button" className="player-button" onClick={music.stop} aria-label={t("music.stop")}>
      <StopIcon size={ICON - 4} />
    </button>
  );
}

/** The transport in the order the film's player has it, with the button that
    stops for good after the one that goes to the next song. */
export function Transport({
  music,
  greyedWhenNone,
  withStop = true,
  disc,
}: {
  music: Music;
  greyedWhenNone?: boolean;
  /** Left out where the stop button stands elsewhere. */
  withStop?: boolean;
  /** The play button drawn as a disc. */
  disc?: boolean;
}) {
  return (
    <>
      <SongStepButton music={music} back greyedWhenNone={greyedWhenNone} />
      <SecondsButton music={music} back />
      <PlayButton music={music} disc={disc} />
      <SecondsButton music={music} back={false} />
      <SongStepButton music={music} back={false} greyedWhenNone={greyedWhenNone} />
      {withStop && <StopButton music={music} />}
    </>
  );
}

export function HeartButton({ id }: { id: string }) {
  const { t } = useSettings();
  const { liked, setLiked } = useLiking(id);
  const label = t(liked ? "card.unfavourite" : "card.favourite");
  return (
    <button
      type="button"
      className={`player-button${liked ? " player-button-lit" : ""}`}
      aria-pressed={liked}
      aria-label={label}
      onClick={() => setLiked(!liked)}
    >
      <HeartIcon size={ICON} filled={liked} />
    </button>
  );
}

export function Volume({ music }: { music: Music }) {
  const { t } = useSettings();
  return (
    <SoundControl
      loudness={music.loudness.volume}
      muted={music.loudness.muted}
      onMuted={music.setMuted}
      onLoudness={music.setVolume}
      t={t}
    />
  );
}

/** Shuffle and repeat, lit when they are on. */
export function Ways({ music }: { music: Music }) {
  const { t } = useSettings();
  const { shuffle, repeat } = music.queue;
  return (
    <>
      <button
        type="button"
        className={`player-button${shuffle ? " player-button-lit" : ""}`}
        onClick={music.toggleShuffle}
        aria-pressed={shuffle}
        aria-label={t("music.shuffle")}
      >
        <ShuffleIcon size={ICON} />
      </button>
      <button
        type="button"
        className={`player-button${repeat !== "off" ? " player-button-lit" : ""}`}
        onClick={music.cycleRepeat}
        aria-label={t(`music.repeat.${repeat}`)}
      >
        <RepeatIcon one={repeat === "one"} size={ICON} />
      </button>
    </>
  );
}

export function QueueButton({ lit = false }: { lit?: boolean }) {
  const { t } = useSettings();
  const nowPlaying = useNowPlayingPage();
  return (
    <button
      type="button"
      className={`player-button${lit ? " player-button-open" : ""}`}
      onClick={nowPlaying.open}
      aria-label={t("music.queue")}
    >
      <QueueIcon size={ICON} />
    </button>
  );
}

/**
 * How far into the song, with the time already played at one end and the
 * time left at the other, drawn as the bar of a film is: a hand on it swells
 * it and shows the time under the pointer, and while it is held the song
 * stays where it is and letting go is the move.
 */
export function Rail({ music }: { music: Music }) {
  const { t } = useSettings();
  const { position, length, loaded } = useMusicTime();
  const rail = useRef<HTMLDivElement>(null);
  const [hovered, setHovered] = useState<number | null>(null);
  const [held, setHeld] = useState<number | null>(null);
  const [railWidth, setRailWidth] = useState(0);

  const shareAt = useCallback((clientX: number): number | null => {
    const bar = rail.current?.getBoundingClientRect();
    if (!bar || bar.width <= 0) {
      return null;
    }
    setRailWidth(bar.width);
    return Math.min(1, Math.max(0, (clientX - bar.left) / bar.width));
  }, []);

  /* Followed on the window rather than on the bar: a finger that leaves the
     bar while still held down is still dragging. */
  const dragging = held !== null;
  useEffect(() => {
    if (!dragging) {
      return;
    }
    const moved = (event: PointerEvent) => {
      const share = shareAt(event.clientX);
      if (share !== null) {
        setHeld(share);
        setHovered(share);
      }
    };
    const letGo = (event: PointerEvent) => {
      const share = shareAt(event.clientX);
      if (share !== null && length > 0) {
        music.seek(share * length);
      }
      setHeld(null);
      setHovered(null);
    };
    window.addEventListener("pointermove", moved);
    window.addEventListener("pointerup", letGo);
    window.addEventListener("pointercancel", letGo);
    return () => {
      window.removeEventListener("pointermove", moved);
      window.removeEventListener("pointerup", letGo);
      window.removeEventListener("pointercancel", letGo);
    };
  }, [dragging, shareAt, length, music]);

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const by = event.key === "ArrowRight" ? KEY_STEP : event.key === "ArrowLeft" ? -KEY_STEP : 0;
    if (by !== 0) {
      event.preventDefault();
      music.seek(Math.min(Math.max(position + by, 0), length));
    }
  };

  const played = held ?? (length > 0 ? Math.min(1, position / length) : 0);
  const shown = held !== null ? held * length : position;
  const previewed = hovered !== null && length > 0 ? hovered * length : null;
  const { left: previewLeft } = previewPlace(TIME_ACROSS, hovered ?? 0, railWidth);

  return (
    <div className="player-seek">
      <span className="player-seek-end">
        <span className="player-clock">{asClock(shown)}</span>
      </span>
      <div
        className="player-rail"
        data-dragging={dragging ? "yes" : undefined}
        ref={rail}
        role="slider"
        tabIndex={0}
        aria-label={t("music.position")}
        aria-valuemin={0}
        aria-valuemax={Math.round(length)}
        aria-valuenow={Math.round(shown)}
        aria-valuetext={asClock(shown)}
        onKeyDown={onKeyDown}
        onPointerDown={(event) => {
          const share = shareAt(event.clientX);
          if (share !== null) {
            setHeld(share);
            setHovered(share);
          }
        }}
        onPointerMove={(event) => setHovered(shareAt(event.clientX))}
        onPointerLeave={() => {
          if (!dragging) {
            setHovered(null);
          }
        }}
      >
        {previewed !== null && (
          <div className="player-preview" style={{ left: `${previewLeft}px` }} aria-hidden="true">
            <span className="player-preview-time">{asClock(previewed)}</span>
          </div>
        )}
        <span className="player-rail-fill">
          <span className="player-rail-track" />
          <span
            className="player-rail-held"
            style={{ width: `${(length > 0 ? Math.min(1, loaded / length) : 0) * 100}%` }}
          />
          <span className="player-rail-played" style={{ width: `${played * 100}%` }} />
        </span>
        <span className="player-rail-handle" style={{ left: `${played * 100}%` }} />
      </div>
      <span className="player-seek-end">
        <span className="player-clock">{length > 0 ? `-${asClock(Math.max(length - shown, 0))}` : ""}</span>
      </span>
    </div>
  );
}
