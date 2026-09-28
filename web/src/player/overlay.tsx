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
  Card,
  PlaybackChapter,
  PlaybackSegment,
  PlaybackTrack,
  Work,
} from "../api";
import type { Appearance } from "./appearance";
import type { Arrangement, Control, Zone } from "./arrangement";
import { chapterStep } from "./chapters";
import { asClock } from "./clock";
import { Drawer, SHEETS } from "./drawer";
import type { SheetName } from "./drawer";
import type { Playback } from "./engine";
import type { Fullscreen } from "./fullscreen";
import {
  AboutIcon,
  AudioIcon,
  BackIcon,
  CastIcon,
  ChaptersIcon,
  CornerIcon,
  FullscreenIcon,
  HeartIcon,
  NextChapterIcon,
  NextEpisodeIcon,
  PauseIcon,
  PlayIcon,
  PreviousChapterIcon,
  PreviousEpisodeIcon,
  StepBackIcon,
  StepOnIcon,
  SubtitlesIcon,
  UpNextIcon,
  VolumeIcon,
} from "./icons";
import { GearIcon } from "../icons";
import type { Mark } from "./logo";
import { captionOf } from "../readable";
import { Panels } from "./panels";
import { Seek } from "./seek";
import type { Panel, Shape, Turn } from "./panels";
import { useMarks } from "../marks";
import { markTheOpening } from "./opening";
import { FADES_AFTER_MS } from "./settings";
import type { PlayerSettings } from "./settings";

/* How large an icon is drawn, read from the stylesheet so that the sizes live
   in one place and a viewer changing them later changes them everywhere. */
const ICON = 27;
const PLAY_ICON = 34;

/**
 * Whether a key is somebody typing, in a box, rather than driving the film.
 *
 * Nothing else keeps its keys: the arrows are the bar and the sound, and the
 * space bar is play and pause, whatever was pressed last. A slider or a
 * button pressed with the mouse keeps the keyboard, and it used to take
 * them: the arrows moved the sound slider by a hundredth without the player
 * hearing of it, and the space bar pressed the last button again.
 */
function isTypedIn(target: EventTarget | null): boolean {
  if (target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement) {
    return true;
  }
  return target instanceof HTMLInputElement && target.type !== "range";
}

/** How large what a key did is drawn in the middle of the picture. */
const KEY_SAID_ICON = 46;

/** How long the sound stays said down the side after the last press. */
const SOUND_SAID_FOR_MS = 1500;

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
  turn: Turn;
  onTurn: (turn: Turn) => void;
  /** What goes fullscreen, which is the picture and its controls together. */
  stage: React.RefObject<HTMLDivElement | null>;
  /** Whether the screen is filled, and how to ask for it either way. The
   *  picture answers to a double click too, so neither owns it. */
  fullscreen: Fullscreen;
  panel: Panel | null;
  onPanel: (panel: Panel | null) => void;
  /** Opens the playback diagnostics, a window of their own beside the
   *  panels rather than one of them. */
  onFacts: () => void;
  /** Opens the window where an administrator corrects the opening and
   *  closing titles. Absent for anybody else, who is offered no such line. */
  onSegments?: () => void;
  onClose: () => void;
  /** Steps to the episode after or before this one. Absent where there is
   *  none, which is when the button that asks for it draws nothing. */
  onNextEpisode?: () => void;
  onPreviousEpisode?: () => void;
  /** Steps straight to any episode of the series, chosen by hand from the
   *  "up next" sheet. Absent for anything that is not an episode. */
  onSelectEpisode?: (episode: { id: string; source_id: string | null }) => void;
  /** What a track is called on screen, which the player works out. */
  naming: (track: PlaybackTrack) => string;
  /** Which language the interface is speaking, for the clock on the wall. */
  language: string;
  t: (key: string, values?: Record<string, string | number>) => string;
  /** How far the two step buttons, and the arrow keys with them, jump, in
   *  seconds, as the viewer chose. */
  steps: { back: number; on: number };
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

export function Overlay(props: Props) {
  const { playback, stage, fullscreen } = props;
  const { at, length, playing } = playback;

  /* Whether the controls have faded out. Kept here rather than in the
     stylesheet alone because the panels and the bar answer to it too: a menu
     left open behind a faded bar is a menu nobody can shut. */
  const [away, setAway] = useState(false);
  /* Said to the opening being followed, since controls going away uncover
     the picture and the way the browser draws it can change with that. */
  const wasAway = useRef(away);
  useEffect(() => {
    if (wasAway.current !== away) {
      wasAway.current = away;
      markTheOpening(playback.video.current, away ? "controls_away" : "controls_back");
    }
  }, [away, playback.video]);
  const stir = useRef<() => void>(() => {});
  /* The sound said down the side of the picture for a moment, when the keys
     change it: the one change a viewer makes without looking at anything,
     and the one they want to see the end of. */
  const [soundSaid, setSoundSaid] = useState(false);
  const soundSaidFor = useRef(0);
  const saySound = useRef(() => {
    setSoundSaid(true);
    window.clearTimeout(soundSaidFor.current);
    soundSaidFor.current = window.setTimeout(() => setSoundSaid(false), SOUND_SAID_FOR_MS);
  });
  useEffect(() => () => window.clearTimeout(soundSaidFor.current), []);
  /* What a key just did to the film, said in the middle of the picture. Only
     for a key: a hand on the mouse is already looking at the button it
     pressed, while a key is pressed looking at the film. Counted, so that a
     second press says it again from the start. */
  const [keyed, setKeyed] = useState<{ what: Keyed; count: number } | null>(null);
  const sayKey = useRef((what: Keyed) =>
    setKeyed((was) => ({ what, count: (was?.count ?? 0) + 1 })),
  );
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
      if (isTypedIn(event.target)) {
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
          // Held from whatever has the focus: a button would be pressed again.
          event.preventDefault();
          sayKey.current(playback.playing ? "pause" : "play");
          playback.playOrPause();
          break;
        case "ArrowLeft":
        case "ArrowRight":
          // Held from the page: the bar answers to these as a slider and would
          // scroll what is behind it otherwise.
          event.preventDefault();
          sayKey.current(event.key === "ArrowLeft" ? "back" : "on");
          playback.stepBy(event.key === "ArrowLeft" ? -props.steps.back : props.steps.on);
          break;
        /* The sound alone, without bringing the controls back over the film:
           what it is now is said down the side instead. */
        case "ArrowUp":
        case "ArrowDown":
          event.preventDefault();
          loudness(event.key === "ArrowUp" ? 0.05 : -0.05);
          saySound.current();
          return;
        case "m":
          playback.setMuted(!playback.muted);
          saySound.current();
          return;
        case "f":
          fullscreen.toggle();
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
    <>
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
              onSelectEpisode={props.onSelectEpisode}
            />
          )}
          <Seek
            playback={playback}
            thumbnails={thumbnails}
            previewScale={props.settings.previewScale}
            turn={props.turn}
            before={<ZoneOnTheBar zone="before_bar" surroundings={surroundings} />}
            after={<ZoneOnTheBar zone="after_bar" surroundings={surroundings} />}
            t={props.t}
          />
          <div className="player-row">
            <Place zone="bottom_left" surroundings={surroundings} />
            <Place zone="bottom_right" surroundings={surroundings} />
          </div>
        </div>
      </div>
      <SoundSaid playback={playback} shown={soundSaid} />
      {keyed && (
        <KeySaid
          key={keyed.count}
          what={keyed.what}
          steps={props.steps}
          onDone={() => setKeyed(null)}
        />
      )}
    </>
  );
}

/**
 * How loud, down the right of the picture, the way a television says it:
 * the sound bar of the controls stood on its end, with the number above it
 * and what it sounds like under it.
 *
 * On every press of a sound key, controls up or not: it is where the eye
 * looks for the answer to that key. Always there and faded rather than made
 * on each press, so it comes and goes without a jump.
 */
function SoundSaid({ playback, shown }: { playback: Playback; shown: boolean }) {
  const loud = playback.muted ? 0 : playback.loudness;
  return (
    <div
      className="player-sound-said"
      data-shown={shown ? "yes" : "no"}
      style={{ ["--share" as string]: `${loud}` }}
      aria-hidden="true"
    >
      <span className="player-sound-said-number">{Math.round(loud * 100)}</span>
      <span className="player-sound-said-rail" />
      <VolumeIcon level={levelOf(loud)} size={22} />
    </div>
  );
}

/** What a key can do to the film that is said in the middle of it. */
type Keyed = "play" | "pause" | "back" | "on";

/** What a key just did, in the middle of the picture: a disc that comes up
 *  and fades away by itself, and is gone once it has. */
function KeySaid({
  what,
  steps,
  onDone,
}: {
  what: Keyed;
  steps: { back: number; on: number };
  onDone: () => void;
}) {
  return (
    <div className="player-key-said" aria-hidden="true" onAnimationEnd={onDone}>
      {what === "play" && <PlayIcon size={KEY_SAID_ICON} />}
      {what === "pause" && <PauseIcon size={KEY_SAID_ICON} />}
      {what === "back" && <StepBackIcon seconds={steps.back} size={KEY_SAID_ICON} />}
      {what === "on" && <StepOnIcon seconds={steps.on} size={KEY_SAID_ICON} />}
    </div>
  );
}

/** The heart, read from and written to the marks every other screen shares:
 *  liked here is liked on the card the viewer walks back to. */
function Favourite({ card, t }: { card: Card; t: Props["t"] }) {
  const marks = useMarks();
  const favourite = marks.favouriteOf(card);
  return (
    <button
      className={`player-button${favourite ? " player-button-lit" : ""}`}
      onClick={() => marks.setFavourite(card, !favourite)}
      aria-pressed={favourite}
      aria-label={t(favourite ? "player.unfavourite" : "player.favourite")}
    >
      <HeartIcon filled={favourite} size={ICON} />
    </button>
  );
}

/** What the sound icon shows for a share of the full sound. */
function levelOf(loud: number): "off" | "low" | "middling" | "high" {
  return loud === 0 ? "off" : loud < 0.34 ? "low" : loud < 0.67 ? "middling" : "high";
}

/** What every control is handed, which is the player and its surroundings. */
export interface Surroundings extends Props {
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

    case "title": {
      // An episode is never the same thing as its mark: the mark is the
      // series, drawn once for all of them, and what tells them apart is its
      // number and its own name, so that is said here whether or not the
      // series has a mark at all.
      if (surroundings.work.kind === "episode") {
        return <span className="player-title">{captionOf(surroundings.work, t)}</span>;
      }
      // Written out only when there is no mark to show instead: a wordmark
      // and the same words beside it is the title twice.
      return surroundings.mark.url ? null : (
        <span className="player-title">{surroundings.mark.words}</span>
      );
    }

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
          onClick={() => playback.stepBy(-surroundings.steps.back)}
          aria-label={t("player.step_back", { seconds: surroundings.steps.back })}
        >
          <StepBackIcon seconds={surroundings.steps.back} size={ICON} />
        </button>
      );

    case "step_on":
      return (
        <button
          className="player-button"
          onClick={() => playback.stepBy(surroundings.steps.on)}
          aria-label={t("player.step_on", { seconds: surroundings.steps.on })}
        >
          <StepOnIcon seconds={surroundings.steps.on} size={ICON} />
        </button>
      );

    case "previous_chapter":
    case "next_chapter": {
      if (surroundings.chapters.length === 0) {
        return null;
      }
      const back = control === "previous_chapter";
      const step = chapterStep(surroundings.chapters, surroundings.at, surroundings.length, back);
      return (
        <button
          className="player-button"
          onClick={() => playback.goTo(step.to)}
          disabled={!step.possible}
          aria-label={t(back ? "player.previous_chapter" : "player.next_chapter")}
        >
          {back ? <PreviousChapterIcon size={ICON} /> : <NextChapterIcon size={ICON} />}
        </button>
      );
    }

    case "previous_episode":
      return surroundings.onPreviousEpisode ? (
        <button
          className="player-button"
          onClick={surroundings.onPreviousEpisode}
          aria-label={t("player.previous_episode")}
        >
          <PreviousEpisodeIcon size={ICON} />
        </button>
      ) : null;

    case "next_episode":
      return surroundings.onNextEpisode ? (
        <button
          className="player-button"
          onClick={surroundings.onNextEpisode}
          aria-label={t(surroundings.work.kind === "episode" ? "player.next_episode" : "player.next")}
        >
          <NextEpisodeIcon size={ICON} />
        </button>
      ) : null;

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
      return surroundings.work.card ? <Favourite card={surroundings.work.card} t={t} /> : null;

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

    /* The sheets, each its own button in the row rather than a strip of
       words under it: a row of its own is a band of film given up whether or
       not anything is open. Pressing the open one again folds it away. Only
       a series has another episode to list, so that one sheet's tab draws
       nothing on a film. */
    case "info":
    case "chapters":
    case "cast":
    case "episodes": {
      if (control === "episodes" && surroundings.work.kind !== "episode") {
        return null;
      }
      const Drawn = {
        info: AboutIcon,
        chapters: ChaptersIcon,
        cast: CastIcon,
        episodes: UpNextIcon,
      }[control];
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
          <GearIcon size={ICON} />
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
        <VolumeIcon level={levelOf(loud)} size={ICON} />
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
  segments,
  t,
}: {
  playback: Playback;
  /** What is offered to skip, which a correction made during the film
   *  changes before the next plan is asked for. */
  segments: PlaybackSegment[];
  t: (key: string, values?: Record<string, string | number>) => string;
}) {
  const inside = theStretchAt(segments, playback.at);
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
