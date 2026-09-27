/*
 * The panels a button of the player opens over the picture: the words, the
 * sound, the settings and every list reached from them.
 *
 * Each panel says what it is, a heading, what it leads back to and its lines,
 * and one piece draws whichever is open. That is what keeps where a panel
 * stands, how it shuts and how it goes back one thing rather than eight.
 */

import { useEffect, useState } from "react";
import type { WideGamutChoice } from "../api";
import { BACKGROUNDS, COLOURS, DEFAULT_APPEARANCE, EDGES, HEIGHTS, SIZES } from "./appearance";
import { AUTOMATIC, CODECS, codecName } from "./codec";
import type { SheetName } from "./drawer";
import { SPEEDS } from "./engine";
import type { Playback } from "./engine";
import { BackIcon, ChosenIcon, IntoIcon } from "./icons";
import type { Surroundings } from "./overlay";
import { AS_IT_IS, QUALITIES, qualityName } from "./quality";
import type { Wording } from "../readable";

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
  | "settings.wide_gamut"
  | "settings.shape"
  | "settings.turn"
  | "settings.repeat"
  | "settings.words_offset"
  | SheetName;

/** How the picture is fitted into the screen. */
export const SHAPES = ["auto", "cover", "stretch"] as const;
export type Shape = (typeof SHAPES)[number];

/** How far the picture is turned, clockwise. For a video of one's own filmed
 *  with the telephone the wrong way round, which no film ever is. */
export const TURNS = [0, 90, 180, 270] as const;
export type Turn = (typeof TURNS)[number];

/** How far the words can be shifted, and by how much at a time, in seconds. */
const OFFSET_STEP = 0.5;
const OFFSET_FURTHEST = 15;

/** What can be done with a film's HDR for this film alone, the account's own
 *  choice first. */
const WIDE_GAMUT_HERE: [WideGamutChoice | null, string][] = [
  [null, "player.wide_gamut.account"],
  ["never_convert", "player.wide_gamut.keep"],
  ["always_convert", "player.wide_gamut.convert"],
];

/** How far the words are shifted, as a viewer reads it: signed, to a tenth of
 *  a second. */
export function offsetSaid(seconds: number): string {
  return `${seconds > 0 ? "+" : ""}${seconds.toFixed(1)} s`;
}

/** The shift after one press of a nudge button, never past either end. */
export function nudged(offset: number, by: number): number {
  return Math.min(OFFSET_FURTHEST, Math.max(-OFFSET_FURTHEST, offset + by));
}

/** A turn as a viewer reads it: nothing at all, or how far. */
function turnName(turn: Turn, t: Wording): string {
  return turn === 0 ? t("player.turn.none") : t("player.turn.degrees", { degrees: turn });
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
        value={offsetSaid(draft)}
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
export function Panels({ surroundings }: { surroundings: Surroundings }) {
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
                  value={offsetSaid(playback.wordsOffset)}
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
              changed={surroundings.shape !== "auto"}
              into
              onPick={() => onPanel("settings.shape")}
            />
            {/* Only for a video of one's own: a film is never shot sideways,
                and a line nobody needs is a line in everybody's way. */}
            {surroundings.work.kind === "video" && (
              <Line
                label={t("player.turn")}
                value={turnName(surroundings.turn, t)}
                changed={surroundings.turn !== 0}
                into
                onPick={() => onPanel("settings.turn")}
              />
            )}
            <Line
              label={t("player.speed")}
              value={`${playback.speed}×`}
              changed={playback.speed !== 1}
              into
              onPick={() => onPanel("settings.speed")}
            />
            <Line
              label={t("player.quality")}
              value={qualityName(playback.quality, t("player.quality.as_it_is"))}
              changed={playback.quality.key !== AS_IT_IS.key}
              into
              onPick={() => onPanel("settings.quality")}
            />
            <Line
              label={t("player.codec")}
              value={codecName(playback.codec, t("player.codec.auto"))}
              changed={playback.codec.key !== AUTOMATIC.key}
              into
              onPick={() => onPanel("settings.codec")}
            />
            {plan.wide_gamut && (
              <Line
                label={t("player.wide_gamut")}
                value={t(plan.wide_gamut.converted ? "player.wide_gamut.converted" : "player.wide_gamut.kept")}
                changed={playback.wideGamut !== null}
                into
                onPick={() => onPanel("settings.wide_gamut")}
              />
            )}
            <Line
              label={t("player.repeat")}
              value={t(playback.repeat ? "player.repeat.film" : "player.repeat.none")}
              changed={playback.repeat}
              into
              onPick={() => onPanel("settings.repeat")}
            />
            <Line
              label={t("player.words_offset")}
              value={offsetSaid(playback.wordsOffset)}
              changed={playback.wordsOffset !== 0}
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
            <Line
              label={t("facts.title")}
              onPick={() => {
                surroundings.onFacts();
                shut();
              }}
            />
            {surroundings.onSegments && (
              <Line
                label={t("segments.open")}
                onPick={() => {
                  surroundings.onSegments?.();
                  shut();
                }}
              />
            )}
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

    case "settings.turn":
      return {
        title: t("player.turn"),
        from: "settings",
        lines: TURNS.map((turn) => (
          <Line
            key={turn}
            label={turnName(turn, t)}
            chosen={surroundings.turn === turn}
            onPick={() => surroundings.onTurn(turn)}
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

    case "settings.wide_gamut": {
      const handling = plan.wide_gamut;
      return {
        title: t("player.wide_gamut"),
        from: "settings",
        lines:
          handling && !handling.follows_choice ? (
            /* Nothing to offer: whatever is picked, the film comes out the
               same, and a choice that changes nothing is worse than none. */
            <p className="player-menu-why">
              {t(handling.converted ? "player.wide_gamut.imposed_converted" : "player.wide_gamut.imposed_kept")}
            </p>
          ) : (
            <>
              <p className="player-menu-why">{t("player.wide_gamut.why")}</p>
              {WIDE_GAMUT_HERE.map(([choice, wording]) => (
                <Line
                  key={wording}
                  label={t(wording)}
                  chosen={playback.wideGamut === choice}
                  onPick={() => playback.setWideGamut(choice)}
                />
              ))}
            </>
          ),
      };
    }

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
      const move = (by: number) => playback.setWordsOffset(nudged(playback.wordsOffset, by));
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
                {offsetSaid(playback.wordsOffset)}
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

    // The drawer's sheets, which the drawer draws.
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
