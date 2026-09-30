/*
 * The sound as the player draws it: the button that mutes and, beside it, the
 * slider that sets it. Shared by the player of films and the player of music,
 * which are one design with two engines behind it.
 */

import { VolumeIcon } from "./icons";
import type { Wording } from "../readable";

/** What the sound icon shows for a share of the full sound. */
export function levelOf(loud: number): "off" | "low" | "middling" | "high" {
  return loud === 0 ? "off" : loud < 0.34 ? "low" : loud < 0.67 ? "middling" : "high";
}

/** How large the icon of a button is drawn, read from the stylesheet's own
 *  sizes so that the two players agree on it. */
export const ICON = 27;

/** The sound, always out where a hand can reach it rather than behind a button. */
export function SoundControl({
  loudness,
  muted,
  onMuted,
  onLoudness,
  t,
}: {
  loudness: number;
  muted: boolean;
  onMuted: (muted: boolean) => void;
  onLoudness: (loudness: number) => void;
  t: Wording;
}) {
  const loud = muted ? 0 : loudness;
  return (
    <span className="player-sound">
      <button
        type="button"
        className="player-button"
        onClick={() => onMuted(!muted)}
        aria-label={t(muted ? "player.unmute" : "player.mute")}
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
          onChange={(event) => onLoudness(Number(event.target.value))}
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
