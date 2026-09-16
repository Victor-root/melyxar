/*
 * Everything a viewer sees over the film, and nothing else.
 *
 * All of it sits on the picture. A film is the one thing in this interface
 * that wants the whole surface, and every strip of controls beside it rather
 * than over it is a strip of film missing. It fades out when a hand stops
 * moving and comes back the moment one does, which is what every player in the
 * world does and the only arrangement that lets somebody watch a film.
 *
 * Nothing here touches the element. The bar asks the engine where to put the
 * film, the buttons ask it to start or stop, the sound is set through it: this
 * file knows where things are drawn and what they are called, and the engine
 * knows what a film is. That is what lets this be rewritten from nothing.
 *
 * What goes where is read from an arrangement rather than written into the
 * drawing below. Each control is a name, each place is a name, and drawing the
 * player is walking one list per place. A viewer is going to hide a button
 * they never press and move another one, and the difference between that being
 * a setting and that being a rewrite is whether the layout was ever data.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import type { PlaybackChapter, PlaybackThumbnails, PlaybackTrack } from "../api";
import type { Arrangement, Control, Zone } from "./arrangement";
import { A_STEP, SPEEDS } from "./engine";
import type { Playback } from "./engine";
import {
  AudioIcon,
  BackIcon,
  ChosenIcon,
  CornerIcon,
  FullscreenIcon,
  HeartIcon,
  IntoIcon,
  NextChapterIcon,
  PauseIcon,
  PlayIcon,
  PreviousChapterIcon,
  SettingsIcon,
  StepBackIcon,
  StepOnIcon,
  SubtitlesIcon,
  VolumeIcon,
} from "./icons";
import type { Mark } from "./logo";
import { QUALITIES, qualityName } from "./quality";
import { FADES_AFTER_MS } from "./settings";
import type { PlayerSettings } from "./settings";

/** Which panel is open, if any. Only ever one: they all cover the picture. */
export type Panel =
  | "subtitles"
  | "audio"
  | "settings"
  | "settings.speed"
  | "settings.quality"
  | "settings.shape"
  | "settings.repeat"
  | "settings.words_offset"
  | "facts";

/** How the picture is fitted into the screen. */
export const SHAPES = ["auto", "cover", "stretch"] as const;
export type Shape = (typeof SHAPES)[number];

/* How large an icon is drawn, read from the stylesheet so that the sizes live
   in one place and a viewer changing them later changes them everywhere. */
const ICON = 27;
const PLAY_ICON = 34;

/** How far the words can be shifted, and by how much at a time, in seconds. */
const OFFSET_STEP = 0.5;
const OFFSET_FURTHEST = 15;

interface Props {
  playback: Playback;
  arrangement: Arrangement;
  settings: PlayerSettings;
  onSettings: (change: Partial<PlayerSettings>) => void;
  /** What the corner shows: the film's mark, the server's, or the title. */
  mark: Mark;
  shape: Shape;
  onShape: (shape: Shape) => void;
  /** What goes fullscreen, which is the picture and its controls together. */
  stage: React.RefObject<HTMLDivElement | null>;
  panel: Panel | null;
  onPanel: (panel: Panel | null) => void;
  onClose: () => void;
  /** What a track is called on screen, which the player works out. */
  naming: (track: PlaybackTrack) => string;
  /** Which language the interface is speaking, for the clock on the wall. */
  language: string;
  t: (key: string, values?: Record<string, string | number>) => string;
}

/** A moment of a film, as somebody reads it. */
export function asClock(seconds: number): string {
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

/** When a film started now would end, at the speed it is being played. */
function endsAt(remaining: number, speed: number): Date | null {
  if (!Number.isFinite(remaining) || remaining <= 0 || speed <= 0) {
    return null;
  }
  return new Date(Date.now() + (remaining / speed) * 1000);
}

/** The chapter a moment is inside, and the one before and after it. */
function chapterAround(chapters: PlaybackChapter[], at: number): number {
  let index = -1;
  for (let i = 0; i < chapters.length; i += 1) {
    if (chapters[i].at_second <= at + 0.25) {
      index = i;
    }
  }
  return index;
}

export function Overlay(props: Props) {
  const { playback, stage } = props;
  const { at, length, playing } = playback;

  /* Whether the controls have faded out. Kept here rather than in the
     stylesheet alone because the panels and the bar answer to it too: a menu
     left open behind a faded bar is a menu nobody can shut. */
  const [away, setAway] = useState(false);
  const [fullscreen, setFullscreen] = useState(false);
  const stir = useRef<() => void>(() => {});

  useEffect(() => {
    const tell = () => setFullscreen(document.fullscreenElement !== null);
    tell();
    document.addEventListener("fullscreenchange", tell);
    return () => document.removeEventListener("fullscreenchange", tell);
  }, []);

  /* Shown while anything is moving, while nothing is playing, and while a
     panel is open: a paused film is a film somebody is about to do something
     with, and a viewer reading a list of soundtracks is not idle. */
  const held = !playing || props.panel !== null || props.settings.keepTheControlsUp;
  useEffect(() => {
    const surface = stage.current;
    if (!surface) {
      return;
    }
    let timer = 0;
    const wake = () => {
      setAway(false);
      window.clearTimeout(timer);
      if (!held) {
        timer = window.setTimeout(() => setAway(true), FADES_AFTER_MS);
      }
    };
    stir.current = wake;
    wake();
    surface.addEventListener("pointermove", wake);
    surface.addEventListener("pointerdown", wake);
    return () => {
      window.clearTimeout(timer);
      surface.removeEventListener("pointermove", wake);
      surface.removeEventListener("pointerdown", wake);
    };
  }, [stage, held, playback.pictureKey]);

  const goFullscreen = useCallback(() => {
    if (document.fullscreenElement) {
      void document.exitFullscreen();
    } else {
      void stage.current?.requestFullscreen?.();
    }
  }, [stage]);

  /* The keyboard, which is the other half of every control below. Held here
     rather than on the page so that one place says what a key does, and so
     that a key does exactly what the button beside it does. */
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      // Somebody typing in a box is typing, not driving the film.
      const into = event.target as HTMLElement | null;
      if (into && ["INPUT", "TEXTAREA", "SELECT"].includes(into.tagName)) {
        return;
      }
      const loudness = (by: number) => {
        playback.setMuted(false);
        playback.setLoudness(Math.min(1, Math.max(0, playback.loudness + by)));
      };
      switch (event.key) {
        case "Escape":
          if (props.panel) {
            props.onPanel(null);
          } else if (document.fullscreenElement) {
            void document.exitFullscreen();
          } else {
            props.onClose();
          }
          break;
        case " ":
        case "k":
          event.preventDefault();
          playback.playOrPause();
          break;
        case "ArrowLeft":
        case "ArrowRight":
          // Held from the page: the bar answers to these as a slider and would
          // scroll what is behind it otherwise.
          event.preventDefault();
          playback.stepBy(event.key === "ArrowLeft" ? -A_STEP : A_STEP);
          break;
        case "ArrowUp":
        case "ArrowDown":
          event.preventDefault();
          loudness(event.key === "ArrowUp" ? 0.05 : -0.05);
          break;
        case "f":
          goFullscreen();
          break;
        case "m":
          playback.setMuted(!playback.muted);
          break;
        default:
          return;
      }
      stir.current();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [playback, props, goFullscreen]);

  const chapters = playback.plan?.chapters ?? [];
  const thumbnails = playback.plan?.thumbnails ?? null;
  const surroundings: Surroundings = {
    ...props,
    fullscreen,
    goFullscreen,
    chapters,
    at,
    length,
  };

  return (
    <div
      className="player-overlay"
      data-away={away ? "yes" : "no"}
      // A hand anywhere on the controls keeps them up, which matters most for
      // the one place a pointer rests without moving: a menu being read.
      onPointerDown={(event) => event.stopPropagation()}
    >
      <div className="player-top">
        <Place zone="top_left" surroundings={surroundings} />
        <Place zone="top_right" surroundings={surroundings} />
      </div>

      {props.panel && <Panels surroundings={surroundings} />}

      <div className="player-bottom">
        <Seek surroundings={surroundings} thumbnails={thumbnails} />
        <div className="player-row">
          <Place zone="bottom_left" surroundings={surroundings} />
          <Place zone="bottom_right" surroundings={surroundings} />
        </div>
      </div>
    </div>
  );
}

/** What every control is handed, which is the player and its surroundings. */
interface Surroundings extends Props {
  fullscreen: boolean;
  goFullscreen: () => void;
  chapters: PlaybackChapter[];
  at: number;
  length: number;
}

/** One of the places a control can sit, drawn from the arrangement. */
function Place({ zone, surroundings }: { zone: Zone; surroundings: Surroundings }) {
  const named = surroundings.arrangement[zone] ?? [];
  return (
    <div className={`player-zone player-zone-${zone.replace("_", "-")}`}>
      {named.map((control) => (
        <One key={control} control={control} surroundings={surroundings} />
      ))}
    </div>
  );
}

/**
 * One control, drawn by name.
 *
 * A control the arrangement names but this film has nothing for draws nothing
 * at all: no subtitle button on a film with no subtitles. That is the film's
 * answer and not the viewer's, and leaving the button there to be pressed for
 * an empty list is worse than not offering it.
 */
function One({ control, surroundings }: { control: Control; surroundings: Surroundings }) {
  const { playback, t, panel, onPanel } = surroundings;
  const plan = playback.plan;

  switch (control) {
    case "back":
      return (
        <button
          className="player-button"
          onClick={surroundings.onClose}
          aria-label={t("player.close")}
        >
          <BackIcon size={ICON} />
        </button>
      );

    case "logo":
      return surroundings.mark.url ? (
        <img
          className={`player-mark player-mark-${surroundings.mark.whose.replace("_", "-")}`}
          src={surroundings.mark.url}
          alt={surroundings.mark.words}
        />
      ) : null;

    case "title":
      // Written out only when there is no mark to show instead: a wordmark
      // and the same words beside it is the title twice.
      return surroundings.mark.url ? null : (
        <span className="player-title">{surroundings.mark.words}</span>
      );

    case "play":
      return (
        <button
          className="player-button player-button-play"
          onClick={playback.playOrPause}
          aria-label={t(playback.playing ? "player.pause" : "player.play")}
        >
          {playback.playing ? <PauseIcon size={PLAY_ICON} /> : <PlayIcon size={PLAY_ICON} />}
        </button>
      );

    case "step_back":
      return (
        <button
          className="player-button"
          onClick={() => playback.stepBy(-A_STEP)}
          aria-label={t("player.back_ten")}
        >
          <StepBackIcon seconds={A_STEP} size={ICON} />
        </button>
      );

    case "step_on":
      return (
        <button
          className="player-button"
          onClick={() => playback.stepBy(A_STEP)}
          aria-label={t("player.on_ten")}
        >
          <StepOnIcon seconds={A_STEP} size={ICON} />
        </button>
      );

    case "previous_chapter":
    case "next_chapter": {
      if (surroundings.chapters.length === 0) {
        return null;
      }
      const on = chapterAround(surroundings.chapters, surroundings.at);
      const back = control === "previous_chapter";
      // Back goes to the start of this chapter unless it has only just begun,
      // which is what every player does and what a hand expects: one press to
      // replay the scene, two to reach the one before.
      const justBegun = on >= 0 && surroundings.at - surroundings.chapters[on].at_second < 3;
      const wanted = back ? (justBegun ? on - 1 : on) : on + 1;
      const there = surroundings.chapters[wanted];
      return (
        <button
          className="player-button"
          onClick={() => playback.goTo(there ? there.at_second : back ? 0 : surroundings.length)}
          disabled={back ? on < 0 && surroundings.at < 3 : wanted >= surroundings.chapters.length}
          aria-label={t(back ? "player.previous_chapter" : "player.next_chapter")}
        >
          {back ? <PreviousChapterIcon size={ICON} /> : <NextChapterIcon size={ICON} />}
        </button>
      );
    }

    case "elapsed":
      return <span className="player-clock">{asClock(surroundings.at)}</span>;

    case "remaining":
      return (
        <span className="player-clock">
          {surroundings.length > 0 ? `-${asClock(surroundings.length - surroundings.at)}` : "‎"}
        </span>
      );

    case "ends_at": {
      const when = endsAt(surroundings.length - surroundings.at, playback.speed);
      if (!when) {
        return null;
      }
      return (
        <span className="player-ends">
          {t("player.ends_at", {
            // In the language the interface is speaking rather than the one
            // the machine is set to: somebody reading a French interface on an
            // American laptop is not asking for half past three in the
            // afternoon.
            time: when.toLocaleTimeString(surroundings.language, {
              hour: "2-digit",
              minute: "2-digit",
            }),
          })}
        </span>
      );
    }

    case "favourite":
      return (
        <button
          className={`player-button${playback.favourite ? " player-button-lit" : ""}`}
          onClick={() => playback.setFavourite(!playback.favourite)}
          aria-pressed={playback.favourite}
          aria-label={t(playback.favourite ? "player.unfavourite" : "player.favourite")}
        >
          <HeartIcon filled={playback.favourite} size={ICON} />
        </button>
      );

    case "subtitles":
      if (!plan || plan.subtitles.length === 0) {
        return null;
      }
      return (
        <button
          className={`player-button${plan.chosen_subtitle_id ? " player-button-lit" : ""}${
            panel === "subtitles" ? " player-button-open" : ""
          }`}
          onClick={() => onPanel(panel === "subtitles" ? null : "subtitles")}
          aria-expanded={panel === "subtitles"}
          aria-label={t("work.subtitles")}
        >
          <SubtitlesIcon size={ICON} />
        </button>
      );

    case "audio":
      if (!plan || plan.audio.length < 2) {
        return null;
      }
      return (
        <button
          className={`player-button${panel === "audio" ? " player-button-open" : ""}`}
          onClick={() => onPanel(panel === "audio" ? null : "audio")}
          aria-expanded={panel === "audio"}
          aria-label={t("work.audio")}
        >
          <AudioIcon size={ICON} />
        </button>
      );

    case "volume":
      return <Volume surroundings={surroundings} />;

    case "settings":
      return (
        <button
          className={`player-button${panel?.startsWith("settings") ? " player-button-open" : ""}`}
          onClick={() => onPanel(panel?.startsWith("settings") ? null : "settings")}
          aria-expanded={panel?.startsWith("settings") ?? false}
          aria-label={t("player.settings")}
        >
          <SettingsIcon size={ICON} />
        </button>
      );

    case "corner":
      if (!("requestPictureInPicture" in HTMLVideoElement.prototype)) {
        return null;
      }
      return (
        <button
          className="player-button"
          onClick={playback.intoTheCorner}
          aria-label={t("player.corner")}
        >
          <CornerIcon size={ICON} />
        </button>
      );

    case "fullscreen":
      return (
        <button
          className="player-button"
          onClick={surroundings.goFullscreen}
          aria-label={t(surroundings.fullscreen ? "player.leave_fullscreen" : "player.fullscreen")}
        >
          <FullscreenIcon leaving={surroundings.fullscreen} size={ICON} />
        </button>
      );

    default:
      return null;
  }
}

/** The sound, always out where a hand can reach it rather than behind a button. */
function Volume({ surroundings }: { surroundings: Surroundings }) {
  const { playback, t } = surroundings;
  const loud = playback.muted ? 0 : playback.loudness;
  return (
    <span className="player-sound">
      <button
        className="player-button"
        onClick={() => playback.setMuted(!playback.muted)}
        aria-label={t(playback.muted ? "player.unmute" : "player.mute")}
      >
        <VolumeIcon level={loud === 0 ? "off" : loud < 0.5 ? "low" : "high"} size={ICON} />
      </button>
      <span className="player-loudness" style={{ ["--filled" as string]: `${loud * 100}%` }}>
        <input
          type="range"
          min={0}
          max={1}
          step={0.01}
          value={loud}
          aria-label={t("player.loudness")}
          onChange={(event) => playback.setLoudness(Number(event.target.value))}
        />
      </span>
    </span>
  );
}

/** The bar, its ends, and the little picture above it. */
function Seek({
  surroundings,
  thumbnails,
}: {
  surroundings: Surroundings;
  thumbnails: PlaybackThumbnails | null;
}) {
  const { playback, t, chapters } = surroundings;
  const { at, length, loaded } = playback;
  const rail = useRef<HTMLDivElement>(null);
  /* Where the cursor is along the bar, from nought to one, while it is on it.
     Null the rest of the time, which is what hides the preview. */
  const [hovered, setHovered] = useState<number | null>(null);
  const [railWidth, setRailWidth] = useState(0);
  const [dragging, setDragging] = useState(false);
  /* Whether the hand went anywhere between landing on the bar and coming off
     it. A click and a drag leave the film in the same place and are not the
     same gesture: only this knows which happened. */
  const wasDragged = useRef(false);

  const shareAt = useCallback((clientX: number): number | null => {
    const bar = rail.current?.getBoundingClientRect();
    if (!bar || bar.width <= 0) {
      return null;
    }
    setRailWidth(bar.width);
    return Math.min(1, Math.max(0, (clientX - bar.left) / bar.width));
  }, []);

  const goToShare = useCallback(
    (share: number) => {
      if (length > 0) {
        playback.goTo(share * length);
      }
    },
    [playback, length],
  );

  /* Followed on the window rather than on the bar: a finger that leaves the
     bar while still held down is still dragging, and a bar that stops
     following it there is a bar that jumps back. */
  useEffect(() => {
    if (!dragging) {
      return;
    }
    const moved = (event: PointerEvent) => {
      const share = shareAt(event.clientX);
      if (share !== null) {
        wasDragged.current = true;
        setHovered(share);
        goToShare(share);
      }
    };
    const letGo = () => {
      setDragging(false);
      playback.viewerMoved(wasDragged.current ? "a_drag" : "a_click");
    };
    window.addEventListener("pointermove", moved);
    window.addEventListener("pointerup", letGo);
    window.addEventListener("pointercancel", letGo);
    return () => {
      window.removeEventListener("pointermove", moved);
      window.removeEventListener("pointerup", letGo);
      window.removeEventListener("pointercancel", letGo);
    };
  }, [dragging, shareAt, goToShare, playback]);

  const played = length > 0 ? Math.min(1, at / length) : 0;
  const held = length > 0 ? Math.min(1, loaded / length) : 0;
  const previewed = hovered !== null && length > 0 ? hovered * length : null;
  const spot = thumbnails && previewed !== null ? spotOf(thumbnails, previewed) : null;
  const scale = surroundings.settings.previewScale;
  const across = (thumbnails?.width ?? 0) * scale;
  /* Kept on screen at both ends rather than half off it. */
  const half = across / 2;
  const previewLeft = Math.min(
    Math.max((hovered ?? 0) * railWidth, half),
    Math.max(half, railWidth - half),
  );

  return (
    <div className="player-seek">
      {previewed !== null && (
        <div className="player-preview" style={{ left: `${previewLeft}px` }} aria-hidden="true">
          {spot && thumbnails && (
            <span
              className="player-preview-picture"
              style={{
                width: `${across}px`,
                height: `${thumbnails.height * scale}px`,
                backgroundImage: `url(${thumbnails.url}/${spot.sheet}.jpg)`,
                backgroundSize: `${thumbnails.columns * across}px ${thumbnails.rows * thumbnails.height * scale}px`,
                backgroundPosition: `-${spot.column * across}px -${spot.row * thumbnails.height * scale}px`,
              }}
            />
          )}
          {/* Under the picture rather than written across it: a time on top of
              a dark frame of film is a time nobody can read. */}
          <span className="player-preview-time">{asClock(previewed)}</span>
        </div>
      )}

      <span className="player-clock player-clock-before">
        <ZoneOnTheBar zone="before_bar" surroundings={surroundings} />
      </span>

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
            wasDragged.current = false;
            playback.viewerMoving();
            setDragging(true);
            setHovered(share);
            goToShare(share);
          }
        }}
        onPointerMove={(event) => setHovered(shareAt(event.clientX))}
        onPointerLeave={() => {
          if (!dragging) {
            setHovered(null);
          }
        }}
      >
        <span className="player-rail-track" />
        <span className="player-rail-held" style={{ width: `${held * 100}%` }} />
        <span className="player-rail-played" style={{ width: `${played * 100}%` }} />
        {/* Where the film changes scene. Drawn over the played part as well as
            the rest, so the shape of the film stays readable all the way
            through it. */}
        {length > 0 &&
          chapters
            .filter((chapter) => chapter.at_second > 0 && chapter.at_second < length)
            .map((chapter) => (
              <span
                key={chapter.at_second}
                className="player-rail-chapter"
                style={{ left: `${(chapter.at_second / length) * 100}%` }}
              />
            ))}
        <span className="player-rail-handle" style={{ left: `${played * 100}%` }} />
      </div>

      <span className="player-clock player-clock-after">
        <ZoneOnTheBar zone="after_bar" surroundings={surroundings} />
      </span>
    </div>
  );
}

/** The two ends of the bar, which hold whatever the arrangement puts there. */
function ZoneOnTheBar({ zone, surroundings }: { zone: Zone; surroundings: Surroundings }) {
  return (
    <>
      {(surroundings.arrangement[zone] ?? []).map((control) => (
        <One key={control} control={control} surroundings={surroundings} />
      ))}
    </>
  );
}

/** One line of a menu: what it is called, what it is on, and a tick or an arrow. */
function Line({
  label,
  value,
  chosen,
  into,
  onPick,
}: {
  label: string;
  value?: string;
  chosen?: boolean;
  into?: boolean;
  onPick: () => void;
}) {
  return (
    <button className="player-menu-line" onClick={onPick} role="menuitem">
      <span className="player-menu-tick">{chosen && <ChosenIcon size={18} />}</span>
      <span className="player-menu-label">{label}</span>
      {value !== undefined && <span className="player-menu-value">{value}</span>}
      {into && <IntoIcon size={16} />}
    </button>
  );
}

/** Whatever panel is open, drawn where the button that opens it stands. */
function Panels({ surroundings }: { surroundings: Surroundings }) {
  const { playback, t, panel, onPanel, naming } = surroundings;
  const plan = playback.plan;
  if (!plan) {
    return null;
  }

  const shut = () => onPanel(null);

  if (panel === "subtitles") {
    return (
      <Menu title={t("work.subtitles")} onShut={shut}>
        <Line
          label={t("player.no_subtitle")}
          chosen={!plan.chosen_subtitle_id}
          onPick={() => {
            playback.choose(playback.audioId, null);
            shut();
          }}
        />
        {plan.subtitles.map((track) => (
          <Line
            key={track.id}
            label={naming(track)}
            chosen={plan.chosen_subtitle_id === track.id}
            onPick={() => {
              playback.choose(playback.audioId, track.id);
              shut();
            }}
          />
        ))}
      </Menu>
    );
  }

  if (panel === "audio") {
    return (
      <Menu title={t("work.audio")} onShut={shut}>
        {plan.audio.map((track) => (
          <Line
            key={track.id}
            label={naming(track)}
            chosen={plan.chosen_audio_id === track.id}
            onPick={() => {
              playback.choose(track.id, playback.subtitleId);
              shut();
            }}
          />
        ))}
      </Menu>
    );
  }

  if (panel === "settings") {
    return (
      <Menu title={t("player.settings")} onShut={shut}>
        <Line
          label={t("player.shape")}
          value={t(`player.shape.${surroundings.shape}`)}
          into
          onPick={() => onPanel("settings.shape")}
        />
        <Line
          label={t("player.speed")}
          value={`${playback.speed}×`}
          into
          onPick={() => onPanel("settings.speed")}
        />
        <Line
          label={t("player.quality")}
          value={qualityName(playback.quality, t("player.quality.as_it_is"))}
          into
          onPick={() => onPanel("settings.quality")}
        />
        <Line
          label={t("player.repeat")}
          value={t(playback.repeat ? "player.repeat.film" : "player.repeat.none")}
          into
          onPick={() => onPanel("settings.repeat")}
        />
        <Line
          label={t("player.words_offset")}
          value={`${playback.wordsOffset > 0 ? "+" : ""}${playback.wordsOffset.toFixed(1)} s`}
          into
          onPick={() => onPanel("settings.words_offset")}
        />
        <Line
          label={t("player.keep_controls_up")}
          chosen={surroundings.settings.keepTheControlsUp}
          onPick={() =>
            surroundings.onSettings({
              keepTheControlsUp: !surroundings.settings.keepTheControlsUp,
            })
          }
        />
        <Line label={t("facts.title")} into onPick={() => onPanel("facts")} />
      </Menu>
    );
  }

  if (panel === "settings.shape") {
    return (
      <Menu title={t("player.shape")} onShut={shut} onBack={() => onPanel("settings")}>
        {SHAPES.map((shape) => (
          <Line
            key={shape}
            label={t(`player.shape.${shape}`)}
            chosen={surroundings.shape === shape}
            onPick={() => surroundings.onShape(shape)}
          />
        ))}
      </Menu>
    );
  }

  if (panel === "settings.speed") {
    return (
      <Menu title={t("player.speed")} onShut={shut} onBack={() => onPanel("settings")}>
        {SPEEDS.map((speed) => (
          <Line
            key={speed}
            label={`${speed}×`}
            chosen={playback.speed === speed}
            onPick={() => playback.setSpeed(speed)}
          />
        ))}
      </Menu>
    );
  }

  if (panel === "settings.quality") {
    return (
      <Menu title={t("player.quality")} onShut={shut} onBack={() => onPanel("settings")}>
        {QUALITIES.map((one) => (
          <Line
            key={one.key}
            label={qualityName(one, t("player.quality.as_it_is"))}
            chosen={playback.quality.key === one.key}
            onPick={() => playback.setQuality(one.key)}
          />
        ))}
      </Menu>
    );
  }

  if (panel === "settings.repeat") {
    return (
      <Menu title={t("player.repeat")} onShut={shut} onBack={() => onPanel("settings")}>
        <Line
          label={t("player.repeat.none")}
          chosen={!playback.repeat}
          onPick={() => playback.setRepeat(false)}
        />
        <Line
          label={t("player.repeat.film")}
          chosen={playback.repeat}
          onPick={() => playback.setRepeat(true)}
        />
      </Menu>
    );
  }

  if (panel === "settings.words_offset") {
    const move = (by: number) =>
      playback.setWordsOffset(
        Math.min(OFFSET_FURTHEST, Math.max(-OFFSET_FURTHEST, playback.wordsOffset + by)),
      );
    return (
      <Menu title={t("player.words_offset")} onShut={shut} onBack={() => onPanel("settings")}>
        <p className="player-menu-why">{t("player.words_offset.why")}</p>
        <div className="player-menu-nudge">
          <button className="player-button" onClick={() => move(-OFFSET_STEP)}>
            {"−"}
          </button>
          <span className="player-menu-amount">
            {`${playback.wordsOffset > 0 ? "+" : ""}${playback.wordsOffset.toFixed(1)} s`}
          </span>
          <button className="player-button" onClick={() => move(OFFSET_STEP)}>
            {"+"}
          </button>
        </div>
        <Line label={t("player.words_offset.reset")} onPick={() => playback.setWordsOffset(0)} />
      </Menu>
    );
  }

  return null;
}

/** A panel standing over the picture, above the button that opened it. */
function Menu({
  title,
  onShut,
  onBack,
  children,
}: {
  title: string;
  onShut: () => void;
  onBack?: () => void;
  children: React.ReactNode;
}) {
  return (
    <div className="player-menu" role="menu" aria-label={title}>
      <div className="player-menu-head">
        {onBack && (
          <button className="player-button player-button-small" onClick={onBack}>
            <BackIcon size={18} />
          </button>
        )}
        <span className="player-menu-title">{title}</span>
        <button className="player-button player-button-small" onClick={onShut} aria-label={title}>
          {"×"}
        </button>
      </div>
      <div className="player-menu-body">{children}</div>
    </div>
  );
}
