/*
 * What every panel of the players is made of: its lines, the panel standing
 * over the button that opened it, and the bar that shifts words against what
 * plays. The player of films and the player of music both draw theirs with
 * these, so a panel looks and behaves the same in either.
 */

import { useEffect, useState } from "react";
import { BackIcon, ChosenIcon, IntoIcon } from "./icons";

/** How far something is shifted, as a viewer reads it: signed, to a tenth of
 *  a second. */
export function offsetSaid(seconds: number): string {
  return `${seconds > 0 ? "+" : ""}${seconds.toFixed(1)} s`;
}

/** One line of a menu: what it is called, what it is on, and a tick or an arrow. */
export function Line({
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
 * The bar for shifting words against what plays: the subtitles of a film,
 * the lyrics of a song.
 *
 * Held in a draft of its own while a hand is on it, and only handed over on
 * release: applying it moves every line, which a drag would otherwise ask
 * for many times a second for no reason, since only the place a hand lets go
 * of the bar was ever going to be watched from.
 */
export function OffsetSlider({
  label,
  seconds,
  furthest,
  step,
  onCommit,
}: {
  label: string;
  seconds: number;
  /** How far it goes either way, in seconds. */
  furthest: number;
  step: number;
  onCommit: (seconds: number) => void;
}) {
  const [draft, setDraft] = useState(seconds);
  useEffect(() => setDraft(seconds), [seconds]);
  const commit = () => onCommit(draft);
  return (
    <>
      <Line label={label} value={offsetSaid(draft)} changed={draft !== 0} />
      <div
        className="player-menu-slider"
        role="presentation"
        style={{ ["--share" as string]: `${((draft + furthest) / (2 * furthest)) * 100}` }}
      >
        <input
          type="range"
          min={-furthest}
          max={furthest}
          step={step}
          value={draft}
          aria-label={label}
          onChange={(event) => setDraft(Number(event.target.value))}
          onPointerUp={commit}
          onKeyUp={commit}
          onBlur={commit}
        />
      </div>
    </>
  );
}

/**
 * A panel standing over the picture, above the button that opened it.
 *
 * Nothing shuts it but the three things that already did: the button that
 * opened it, the escape key, and the picture behind it. A cross in the corner
 * of a panel this small is a fourth way of doing what pressing the same button
 * again does, and it costs a corner of every panel.
 */
export function Menu({
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
