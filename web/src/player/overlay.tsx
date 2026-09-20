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
import type {
  PlaybackChapter,
  PlaybackSegment,
  PlaybackThumbnails,
  PlaybackTrack,
  Work,
} from "../api";
import { BACKGROUNDS, COLOURS, DEFAULT_APPEARANCE, EDGES, HEIGHTS, SIZES } from "./appearance";
import type { Appearance } from "./appearance";
import type { Arrangement, Control, Zone } from "./arrangement";
import { asClock } from "./clock";
import { Drawer, SHEETS } from "./drawer";
import type { SheetName } from "./drawer";
import { A_STEP, SPEEDS } from "./engine";
import type { Playback } from "./engine";
import type { Fullscreen } from "./fullscreen";
import {
  AboutIcon,
  AudioIcon,
  BackIcon,
  CastIcon,
  ChaptersIcon,
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
import { CODECS, codecName } from "./codec";
import { FADES_AFTER_MS } from "./settings";
import type { PlayerSettings } from "./settings";
import { Thumbnail } from "./thumbnail";

/**
 * Which panel is open, if any. Only ever one: they all cover the picture.
 *
 * The three sheets of the drawer are among them rather than a state of their
 * own, which is what makes the tabs work: pressing one is opening a panel,
 * and opening any other panel shuts the drawer without either knowing about
 * the other.
 */
export type Panel =
  | "subtitles"
  | "subtitles.size"
  | "subtitles.colour"
  | "subtitles.edge"
  | "subtitles.background"
  | "subtitles.height"
  | "subtitles.offset"
  | "audio"
  | "settings"
  | "settings.speed"
  | "settings.quality"
  | "settings.codec"
  | "settings.shape"
  | "settings.repeat"
  | "settings.words_offset"
  | "facts"
  | SheetName;

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
  /** The film as the library describes it, for the drawer above the bar. */
  work: Work;
  /** What the corner shows: the film's mark, the server's, or the title. */
  mark: Mark;
  shape: Shape;
  onShape: (shape: Shape) => void;
  /** What goes fullscreen, which is the picture and its controls together. */
  stage: React.RefObject<HTMLDivElement | null>;
  /** Whether the screen is filled, and how to ask for it either way. The
   *  picture answers to a double click too, so neither owns it. */
  fullscreen: Fullscreen;
  panel: Panel | null;
  onPanel: (panel: Panel | null) => void;
  onClose: () => void;
  /** What a track is called on screen, which the player works out. */
  naming: (track: PlaybackTrack) => string;
  /** Which language the interface is speaking, for the clock on the wall. */
  language: string;
  t: (key: string, values?: Record<string, string | number>) => string;
  /** How the words look, and how a viewer changes that. Read here rather than
   *  kept beside the panel that shows it, because it is also what draws the
   *  film's own title bar in the theme it belongs to. */
  appearance: Appearance;
  onAppearance: (change: Partial<Appearance>) => void;
}

/** Whether what is open is one of the drawer's three sheets. */
function isASheet(panel: Panel | null): panel is SheetName {
  return panel !== null && (SHEETS as readonly string[]).includes(panel);
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
  const { playback, stage, fullscreen } = props;
  const { at, length, playing } = playback;

  /* Whether the controls have faded out. Kept here rather than in the
     stylesheet alone because the panels and the bar answer to it too: a menu
     left open behind a faded bar is a menu nobody can shut. */
  const [away, setAway] = useState(false);
  const stir = useRef<() => void>(() => {});
  /* Where the button that opened the panel stands, across the picture. A panel
     that always opens at one end of the screen leaves a viewer looking for the
     link between the button they pressed and the list that appeared. */
  const [anchor, setAnchor] = useState<number | null>(null);
  /* Which sheet of the drawer was last read. Kept so the button reopens it
     where it was left, and forgotten between films with everything else. */
  const [lastSheet, setLastSheet] = useState<SheetName>("info");
  useEffect(() => {
    if (isASheet(props.panel)) {
      setLastSheet(props.panel);
    }
  }, [props.panel]);

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

  /* How tall the bottom strip actually is right now, carried onto the stage as
     a custom property so that subtitles, which stand outside this overlay
     entirely, can be lifted to sit just above it. Measured rather than
     guessed: the strip's height changes with the drawer, the screen's width,
     and the row of controls itself, and a number written down here would be
     wrong the moment any of those changed. */
  const bottomBar = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const bar = bottomBar.current;
    const surface = stage.current;
    if (!bar || !surface) {
      return;
    }
    const measure = new ResizeObserver(() => {
      // The content box alone, which a `ResizeObserver` entry reports by
      // default, leaves out the strip's own padding: real space the strip
      // still occupies, and on the very padding a viewer's eye is measuring
      // the gap against. Read off the element itself instead, which is the
      // space it actually takes on the picture.
      //
      // Except the padding above it, which answers to the drawer rather than
      // to the strip: the gradient that darkens the picture behind the row
      // reaches ninety-two pixels higher than the row itself while the
      // drawer is shut, mostly nothing underneath it, and dropping to none
      // at all once the drawer is open. Counted in with the rest, it stood
      // clear of a strip nearly twice its own real height whenever the
      // drawer was shut. Read off the style rather than written down again
      // here, so the two numbers cannot drift apart.
      const paddingTop = Number.parseFloat(getComputedStyle(bar).paddingTop) || 0;
      surface.style.setProperty("--controls-height", `${bar.offsetHeight - paddingTop}px`);
    });
    measure.observe(bar);
    return () => measure.disconnect();
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
          fullscreen.toggle();
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
  }, [playback, props, fullscreen]);

  /* Opened from a button, which says where it stands as it does so. */
  const openFrom = useCallback(
    (panel: Panel | null, from?: HTMLElement | null) => {
      const surface = stage.current?.getBoundingClientRect();
      const button = from?.getBoundingClientRect();
      setAnchor(surface && button ? button.x + button.width / 2 - surface.x : null);
      props.onPanel(panel);
    },
    [stage, props],
  );

  const chapters = playback.plan?.chapters ?? [];
  const thumbnails = playback.plan?.thumbnails ?? null;
  const surroundings: Surroundings = {
    ...props,
    chapters,
    at,
    length,
    anchor,
    openFrom,
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

      {/* All of it inside the bottom strip rather than floating over it: a
          panel standing clear of the controls leaves a band of film between
          the two and reads as two things, when what a viewer sees is one. The
          strip grows upwards because it is anchored to the bottom.

          The sheet sits above the row that opens it rather than under it, so
          that what it says lands on the picture. Under the row it lands at
          the very bottom of the window, which on a wide film is the black
          band the picture does not reach: a sheet you can see through, with
          nothing behind it to see. */}
      <div className="player-bottom" ref={bottomBar}>
        {playback.plan && (
          <Drawer
            work={props.work}
            plan={playback.plan}
            playback={playback}
            showing={isASheet(props.panel) ? props.panel : lastSheet}
            open={isASheet(props.panel)}
            language={props.language}
            t={props.t}
          />
        )}
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
  chapters: PlaybackChapter[];
  at: number;
  length: number;
  /** Where the button that opened the panel stands, across the picture. */
  anchor: number | null;
  openFrom: (panel: Panel | null, from?: HTMLElement | null) => void;
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
          srcSet={surroundings.mark.srcSet ?? undefined}
          /* The widest the box can be, so a screen with fine pixels takes the
             larger of the two and every other screen leaves it alone. */
          sizes="340px"
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
          onClick={(event) =>
            surroundings.openFrom(
              panel === "subtitles" ? null : "subtitles",
              event.currentTarget,
            )
          }
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
          onClick={(event) =>
            surroundings.openFrom(panel === "audio" ? null : "audio", event.currentTarget)
          }
          aria-expanded={panel === "audio"}
          aria-label={t("work.audio")}
        >
          <AudioIcon size={ICON} />
        </button>
      );

    /* The three sheets, each its own button in the row rather than a strip of
       words under it: a row of its own is a band of film given up whether or
       not anything is open. Pressing the open one again folds it away. */
    case "info":
    case "chapters":
    case "cast": {
      const Drawn = { info: AboutIcon, chapters: ChaptersIcon, cast: CastIcon }[control];
      return (
        <button
          className={`player-button${panel === control ? " player-button-open" : ""}`}
          role="tab"
          id={`player-sheet-tab-${control}`}
          aria-selected={panel === control}
          aria-controls="player-sheet"
          aria-label={t(`player.sheet.${control}`)}
          onClick={() => onPanel(panel === control ? null : control)}
        >
          <Drawn size={ICON} />
        </button>
      );
    }

    /* A line between two groups of buttons, so a row of nine reads as what it
       is: what the film is on one side, what is being done with it on the
       other. */
    case "separator":
      return <span className="player-separator" aria-hidden="true" />;

    case "volume":
      return <Volume surroundings={surroundings} />;

    case "settings":
      return (
        <button
          className={`player-button${panel?.startsWith("settings") ? " player-button-open" : ""}`}
          onClick={(event) =>
            surroundings.openFrom(
              panel?.startsWith("settings") ? null : "settings",
              event.currentTarget,
            )
          }
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
          onClick={surroundings.fullscreen.toggle}
          aria-label={t(
            surroundings.fullscreen.filling ? "player.leave_fullscreen" : "player.fullscreen",
          )}
        >
          <FullscreenIcon leaving={surroundings.fullscreen.filling} size={ICON} />
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
        <VolumeIcon
          level={loud === 0 ? "off" : loud < 0.34 ? "low" : loud < 0.67 ? "middling" : "high"}
          size={ICON}
        />
      </button>
      {/* The share is handed over as a bare number rather than as a width, so
          the stylesheet can work out where the handle actually stands: a
          browser keeps its handle inside the track at both ends, so the middle
          of it travels a little less than the whole width. Anything drawn at a
          plain percentage drifts away from it towards the ends. */}
      <span className="player-loudness" style={{ ["--share" as string]: `${loud}` }}>
        <input
          type="range"
          min={0}
          max={1}
          step={0.01}
          value={loud}
          aria-label={t("player.loudness")}
          onChange={(event) => playback.setLoudness(Number(event.target.value))}
        />
        {/* How loud, in the round numbers a person thinks in, standing over the
            handle while a hand is on it. */}
        <span className="player-loudness-said" aria-hidden="true">
          {Math.round(loud * 100)}
        </span>
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
  const { playback, t } = surroundings;
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
  const scale = surroundings.settings.previewScale;
  /* Never wider than the bar it stands on. A viewer can make these larger, and
     a window can be made narrower than the largest of them: past that point it
     is the bar that decides, because a picture wider than the bar cannot be
     kept inside the screen at both ends whatever it is centred on. */
  const wanted = (thumbnails?.width ?? 0) * scale;
  const across = railWidth > 0 ? Math.min(wanted, railWidth) : wanted;
  /* Kept inside the bar at both ends rather than half off the screen. */
  const half = across / 2;
  const previewLeft = Math.min(
    Math.max((hovered ?? 0) * railWidth, half),
    Math.max(half, railWidth - half),
  );

  return (
    <div className="player-seek">
      {/* The two ends of the bar hold whatever the arrangement puts there and
          are sized by it, never by a width written here: a box wider than its
          own words pushes the bar away from one end and not the other, and the
          bar stops being centred between the two. */}
      <span className="player-seek-end">
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
        {/* Inside the bar rather than beside it, because where it stands is a
            place along the bar. Hung on the row instead, it was out by the
            width of the clock to its left: it followed the hand correctly and
            sat beside it the whole way. */}
        {previewed !== null && (
          <div className="player-preview" style={{ left: `${previewLeft}px` }} aria-hidden="true">
            <Thumbnail
              thumbnails={thumbnails}
              seconds={previewed}
              across={across}
              className="player-preview-picture"
            />
            {/* Under the picture rather than written across it: a time on top
                of a dark frame of film is a time nobody can read. */}
            <span className="player-preview-time">{asClock(previewed)}</span>
          </div>
        )}
        <span className="player-rail-fill">
          <span className="player-rail-track" />
          <span className="player-rail-held" style={{ width: `${held * 100}%` }} />
          <span className="player-rail-played" style={{ width: `${played * 100}%` }} />
        </span>
        <span className="player-rail-handle" style={{ left: `${played * 100}%` }} />
      </div>

      <span className="player-seek-end">
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
  changed,
  chosen,
  switched,
  into,
  onPick,
}: {
  label: string;
  value?: string;
  /** Whether the value shown is something other than what a viewer who never
   *  touched this gets: coloured, so a glance at the row it opens from says
   *  whether there is anything on it worth going back to the default for. */
  changed?: boolean;
  /** One of a list, of which exactly one is picked: a tick down the left. */
  chosen?: boolean;
  /** A setting that is on or off on its own: a switch on the right. The two
   *  are not the same question and must not look the same. */
  switched?: boolean;
  into?: boolean;
  /** Left out for a line that only ever says what a value is, with the
   *  control that actually changes it drawn beside it rather than reached
   *  by opening anything: a row rather than a button, with nothing to press. */
  onPick?: () => void;
}) {
  const aSwitch = switched !== undefined;
  const words = (
    <>
      {/* The column is there whether or not this line has a tick in it, so
          every line in a panel starts its wording in the same place. */}
      <span className="player-menu-tick">{chosen && <ChosenIcon size={18} />}</span>
      <span className="player-menu-label">{label}</span>
      {value !== undefined && (
        <span className={`player-menu-value${changed ? " player-menu-value-changed" : ""}`}>
          {value}
        </span>
      )}
      {aSwitch && <Switch on={switched} />}
      {into && <IntoIcon size={16} />}
    </>
  );
  if (!onPick) {
    return <div className="player-menu-line player-menu-line-static">{words}</div>;
  }
  return (
    <button
      className="player-menu-line"
      onClick={onPick}
      role={aSwitch ? "menuitemcheckbox" : "menuitem"}
      aria-checked={aSwitch ? switched : undefined}
    >
      {words}
    </button>
  );
}

/**
 * On or off, as a switch rather than a tick.
 *
 * A tick down the left of a list means "this is the one picked out of these";
 * a setting that stands alone is not one of a list, and drawn the same way it
 * reads as though the others were unpicked. Built to the shape Material's own
 * switch uses, without the tick inside the handle: the handle growing as it
 * moves already says which side it is on.
 */
function Switch({ on }: { on: boolean }) {
  return (
    <span className="player-switch" data-on={on ? "yes" : "no"} aria-hidden="true">
      <span className="player-switch-handle" />
    </span>
  );
}

/**
 * The bar for shifting the words against the picture.
 *
 * Held in a draft of its own while a hand is on it, and only carried over to
 * the film's own words on release: applying it walks every cue on the
 * track, which a drag would otherwise ask for many times a second for no
 * reason, since only the place a hand lets go of the bar was ever going to
 * be watched from.
 */
function SubtitleOffsetSlider({
  playback,
  t,
}: {
  playback: Playback;
  t: (key: string, values?: Record<string, string | number>) => string;
}) {
  const [draft, setDraft] = useState(playback.wordsOffset);
  useEffect(() => setDraft(playback.wordsOffset), [playback.wordsOffset]);
  const commit = () => playback.setWordsOffset(draft);
  return (
    <>
      <Line
        label={t("player.words_offset")}
        value={`${draft > 0 ? "+" : ""}${draft.toFixed(1)} s`}
        changed={draft !== 0}
      />
      <div
        className="player-menu-slider"
        role="presentation"
        style={{ ["--share" as string]: `${((draft + OFFSET_FURTHEST) / (2 * OFFSET_FURTHEST)) * 100}` }}
      >
        <input
          type="range"
          min={-OFFSET_FURTHEST}
          max={OFFSET_FURTHEST}
          step={OFFSET_STEP}
          value={draft}
          aria-label={t("player.words_offset")}
          onChange={(event) => setDraft(Number(event.target.value))}
          onPointerUp={commit}
          onKeyUp={commit}
          onBlur={commit}
        />
      </div>
    </>
  );
}

/** What one open panel is: a heading, what it leads back to, and its lines. */
interface Sheet {
  title: string;
  /** The panel this one was reached from, for the arrow in its heading. */
  from?: Panel;
  lines: React.ReactNode;
  /** Wider than the usual list, for the one panel that carries a second
   *  column beside its lines. */
  wide?: boolean;
}

/**
 * Whatever panel is open, drawn where the button that opens it stands.
 *
 * Each one says what it is rather than drawing itself, and the one panel
 * below draws all of them. That is what keeps where a panel stands, how it
 * shuts and how it goes back one thing rather than eight.
 */
function Panels({ surroundings }: { surroundings: Surroundings }) {
  const { playback, onPanel } = surroundings;
  const plan = playback.plan;
  if (!plan) {
    return null;
  }

  const shut = () => onPanel(null);
  const sheet = sheetFor(surroundings, plan, shut);
  if (!sheet) {
    return null;
  }

  return (
    <Menu
      title={sheet.title}
      onBack={sheet.from && (() => onPanel(sheet.from ?? null))}
      anchor={surroundings.anchor}
      wide={sheet.wide}
    >
      {sheet.lines}
    </Menu>
  );
}

/** Which panel is open and what is in it, with nothing said about where it
 *  is drawn. */
function sheetFor(
  surroundings: Surroundings,
  plan: NonNullable<Playback["plan"]>,
  shut: () => void,
): Sheet | null {
  const { playback, t, panel, onPanel, naming, appearance, onAppearance } = surroundings;

  /** One of the five lists behind how the words look: the same shape five
   *  times over, told apart only by which values it offers and what wording
   *  they answer to. */
  function subtitleLookPanel<T extends string>(
    title: string,
    among: readonly T[],
    value: T,
    wording: string,
    onPick: (value: T) => void,
  ): Sheet {
    return {
      title,
      from: "subtitles",
      lines: among.map((one) => (
        <Line key={one} label={t(`player.${wording}.${one}`)} chosen={value === one} onPick={() => onPick(one)} />
      )),
    };
  }

  switch (panel) {
    case "subtitles": {
      // Left open on a pick rather than shut: a viewer choosing a language is
      // very often about to reach for how it looks, and closing the one panel
      // that shows both would send them back to the button that opened it.
      const dressed = plan.chosen_subtitle_id !== null;
      return {
        title: t("work.subtitles"),
        wide: dressed,
        lines: (
          <div className="player-subtitle-panel">
            <div className="player-subtitle-panel-tracks">
              <Line
                label={t("player.no_subtitle")}
                chosen={!plan.chosen_subtitle_id}
                onPick={() => playback.choose(playback.audioId, null)}
              />
              {plan.subtitles.map((track) => (
                <Line
                  key={track.id}
                  label={naming(track)}
                  chosen={plan.chosen_subtitle_id === track.id}
                  onPick={() => playback.choose(playback.audioId, track.id)}
                />
              ))}
            </div>
            {/* Only while a subtitle is actually chosen: offering to restyle
                words that are not on screen is a column of pickers that do
                nothing. Each opens the same kind of list every other choice
                in the player opens, rather than the browser's own dropdown,
                which nothing here can make look like the rest of it. */}
            {dressed && (
              <div className="player-subtitle-panel-dressing">
                <Line
                  label={t("player.subtitle_size")}
                  value={t(`player.subtitle_size.${appearance.size}`)}
                  changed={appearance.size !== DEFAULT_APPEARANCE.size}
                  into
                  onPick={() => onPanel("subtitles.size")}
                />
                <Line
                  label={t("player.subtitle_colour")}
                  value={t(`player.subtitle_colour.${appearance.colour}`)}
                  changed={appearance.colour !== DEFAULT_APPEARANCE.colour}
                  into
                  onPick={() => onPanel("subtitles.colour")}
                />
                <Line
                  label={t("player.subtitle_edge")}
                  value={t(`player.subtitle_edge.${appearance.edge}`)}
                  changed={appearance.edge !== DEFAULT_APPEARANCE.edge}
                  into
                  onPick={() => onPanel("subtitles.edge")}
                />
                <Line
                  label={t("player.subtitle_background")}
                  value={t(`player.subtitle_background.${appearance.background}`)}
                  changed={appearance.background !== DEFAULT_APPEARANCE.background}
                  into
                  onPick={() => onPanel("subtitles.background")}
                />
                <Line
                  label={t("player.subtitle_height")}
                  value={t(`player.subtitle_height.${appearance.height}`)}
                  changed={appearance.height !== DEFAULT_APPEARANCE.height}
                  into
                  onPick={() => onPanel("subtitles.height")}
                />
                <Line
                  label={t("player.words_offset")}
                  value={`${playback.wordsOffset > 0 ? "+" : ""}${playback.wordsOffset.toFixed(1)} s`}
                  changed={playback.wordsOffset !== 0}
                  into
                  onPick={() => onPanel("subtitles.offset")}
                />
              </div>
            )}
          </div>
        ),
      };
    }

    case "subtitles.size":
      return subtitleLookPanel(t("player.subtitle_size"), SIZES, appearance.size, "subtitle_size", (size) =>
        onAppearance({ size }),
      );
    case "subtitles.colour":
      return subtitleLookPanel(
        t("player.subtitle_colour"),
        COLOURS,
        appearance.colour,
        "subtitle_colour",
        (colour) => onAppearance({ colour }),
      );
    case "subtitles.edge":
      return subtitleLookPanel(t("player.subtitle_edge"), EDGES, appearance.edge, "subtitle_edge", (edge) =>
        onAppearance({ edge }),
      );
    case "subtitles.background":
      return subtitleLookPanel(
        t("player.subtitle_background"),
        BACKGROUNDS,
        appearance.background,
        "subtitle_background",
        (background) => onAppearance({ background }),
      );
    case "subtitles.height": {
      // The fourth is not one more place beside the other three: it is a
      // hand on a slider, and moving it is choosing it, whatever was chosen
      // before. The other three stay exactly what they always were.
      const custom = appearance.height === "custom";
      return {
        title: t("player.subtitle_height"),
        from: "subtitles",
        lines: (
          <>
            {HEIGHTS.filter((one) => one !== "custom").map((one) => (
              <Line
                key={one}
                label={t(`player.subtitle_height.${one}`)}
                chosen={appearance.height === one}
                onPick={() => onAppearance({ height: one })}
              />
            ))}
            <Line
              label={t("player.subtitle_height.custom")}
              value={custom ? `${Math.round(appearance.heightCustom)}%` : undefined}
              changed={custom}
              chosen={custom}
              onPick={() => onAppearance({ height: "custom" })}
            />
            <div
              className="player-menu-slider"
              role="presentation"
              style={{ ["--share" as string]: `${appearance.heightCustom}` }}
            >
              <input
                type="range"
                min={0}
                max={100}
                step={1}
                value={appearance.heightCustom}
                aria-label={t("player.subtitle_height.custom")}
                onChange={(event) =>
                  onAppearance({ height: "custom", heightCustom: Number(event.target.value) })
                }
              />
            </div>
          </>
        ),
      };
    }

    case "subtitles.offset":
      return {
        title: t("player.words_offset"),
        from: "subtitles",
        lines: (
          <>
            <SubtitleOffsetSlider playback={playback} t={t} />
            <p className="player-menu-why">{t("player.words_offset.why")}</p>
            <Line
              label={t("player.words_offset.reset")}
              onPick={() => playback.setWordsOffset(0)}
            />
          </>
        ),
      };

    case "audio":
      return {
        title: t("work.audio"),
        lines: plan.audio.map((track) => (
          <Line
            key={track.id}
            label={naming(track)}
            chosen={plan.chosen_audio_id === track.id}
            onPick={() => {
              playback.choose(track.id, playback.subtitleId);
              shut();
            }}
          />
        )),
      };

    case "settings":
      return {
        title: t("player.settings"),
        lines: (
          <>
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
              label={t("player.codec")}
              value={codecName(playback.codec, t("player.codec.auto"))}
              into
              onPick={() => onPanel("settings.codec")}
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
              switched={surroundings.settings.keepTheControlsUp}
              onPick={() =>
                surroundings.onSettings({
                  keepTheControlsUp: !surroundings.settings.keepTheControlsUp,
                })
              }
            />
            <Line label={t("facts.title")} into onPick={() => onPanel("facts")} />
          </>
        ),
      };

    case "settings.shape":
      return {
        title: t("player.shape"),
        from: "settings",
        lines: SHAPES.map((shape) => (
          <Line
            key={shape}
            label={t(`player.shape.${shape}`)}
            chosen={surroundings.shape === shape}
            onPick={() => surroundings.onShape(shape)}
          />
        )),
      };

    case "settings.speed":
      return {
        title: t("player.speed"),
        from: "settings",
        lines: SPEEDS.map((speed) => (
          <Line
            key={speed}
            label={`${speed}×`}
            chosen={playback.speed === speed}
            onPick={() => playback.setSpeed(speed)}
          />
        )),
      };

    case "settings.quality":
      return {
        title: t("player.quality"),
        from: "settings",
        lines: QUALITIES.map((one) => (
          <Line
            key={one.key}
            label={qualityName(one, t("player.quality.as_it_is"))}
            chosen={playback.quality.key === one.key}
            onPick={() => playback.setQuality(one.key)}
          />
        )),
      };

    case "settings.codec":
      return {
        title: t("player.codec"),
        from: "settings",
        lines: CODECS.map((one) => (
          <Line
            key={one.key}
            label={codecName(one, t("player.codec.auto"))}
            chosen={playback.codec.key === one.key}
            onPick={() => playback.setCodec(one.key)}
          />
        )),
      };

    case "settings.repeat":
      return {
        title: t("player.repeat"),
        from: "settings",
        lines: (
          <>
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
          </>
        ),
      };

    case "settings.words_offset": {
      const move = (by: number) =>
        playback.setWordsOffset(
          Math.min(OFFSET_FURTHEST, Math.max(-OFFSET_FURTHEST, playback.wordsOffset + by)),
        );
      return {
        title: t("player.words_offset"),
        from: "settings",
        lines: (
          <>
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
            <Line
              label={t("player.words_offset.reset")}
              onPick={() => playback.setWordsOffset(0)}
            />
          </>
        ),
      };
    }

    // The facts are a sheet of their own rather than a list of choices, and
    // the player draws them beside this.
    default:
      return null;
  }
}
/**
 * A panel standing over the picture, above the button that opened it.
 *
 * Nothing shuts it but the three things that already did: the button that
 * opened it, the escape key, and the picture behind it. A cross in the corner
 * of a panel this small is a fourth way of doing what pressing the same button
 * again does, and it costs a corner of every panel.
 */
function Menu({
  title,
  onBack,
  anchor,
  wide,
  children,
}: {
  title: string;
  onBack?: () => void;
  /** Where the button that opened it stands, when one did. */
  anchor?: number | null;
  /** Wider than the usual list, for the one panel that carries a second
   *  column beside its lines. */
  wide?: boolean;
  children: React.ReactNode;
}) {
  return (
    <div
      className={`player-menu${anchor == null ? " player-menu-at-the-end" : ""}${wide ? " player-menu-wide" : ""}`}
      role="menu"
      aria-label={title}
      /* Standing over the button that opened it. Held inside the picture at
         both edges by the stylesheet, which knows how wide the panel is and
         how far in things are allowed to come. */
      style={anchor == null ? undefined : { ["--anchor" as string]: `${anchor}px` }}
    >
      <div className="player-menu-head">
        {onBack && (
          <button className="player-button player-button-small" onClick={onBack}>
            <BackIcon size={18} />
          </button>
        )}
        <span className="player-menu-title">{title}</span>
      </div>
      <div className="player-menu-body">{children}</div>
    </div>
  );
}

/**
 * The button that skips the stretch the film is in the middle of.
 *
 * Shown only while the film is actually inside one, and it takes the film to
 * where that stretch ends and nowhere else. Named after what it skips: a
 * button that only says "skip" leaves a viewer guessing what they are about
 * to lose.
 *
 * Drawn beside the controls rather than inside them. The whole set of
 * controls fades out when nobody touches anything, and this one is wanted
 * precisely then: a viewer sitting through an opening is a viewer who has not
 * touched anything for a minute.
 */
export function SkipStretch({
  playback,
  t,
}: {
  playback: Playback;
  t: (key: string, values?: Record<string, string | number>) => string;
}) {
  const inside = theStretchAt(playback.plan?.segments ?? [], playback.at);
  if (!inside) {
    return null;
  }

  return (
    <button
      className="button player-skip"
      onClick={() => playback.goTo(inside.to_second)}
    >
      {t(`player.skip.${inside.kind}`)}
    </button>
  );
}

/**
 * The stretch a film is in the middle of at this second, if it is in one.
 *
 * The first that holds the second, so two that overlap answer the earlier:
 * whichever a viewer entered first is the one they are sitting through.
 */
export function theStretchAt(
  segments: PlaybackSegment[],
  at: number,
): PlaybackSegment | null {
  return (
    segments.find((segment) => at >= segment.from_second && at < segment.to_second) ?? null
  );
}
