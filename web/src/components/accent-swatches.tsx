/*
 * The accent, chosen among the colours offered at a press or any other by
 * hand: for an account's own interface, and for the server's, which every
 * account that chose none wears.
 */

import { TickIcon } from "../icons";
import { OFFERED_ACCENTS } from "../settings";
import { ColourPicker } from "./colour-picker";

export function AccentSwatches({
  colour,
  onPick,
  label,
}: {
  colour: string;
  onPick: (colour: string) => void;
  /** What is being chosen, for whoever cannot see the swatches. */
  label: string;
}) {
  const ownClass = `swatch swatch-own${OFFERED_ACCENTS.includes(colour) ? "" : " swatch-on"}`;
  return (
    <div className="swatches" role="radiogroup" aria-label={label}>
      {OFFERED_ACCENTS.map((offered) => {
        const chosen = offered === colour;
        return (
          <button
            key={offered}
            type="button"
            role="radio"
            aria-checked={chosen}
            aria-label={offered}
            className={`swatch${chosen ? " swatch-on" : ""}`}
            style={{ ["--swatch" as string]: offered }}
            onClick={() => onPick(offered)}
          >
            {chosen && <TickIcon size={14} />}
          </button>
        );
      })}
      <ColourPicker colour={colour} onPick={onPick} className={ownClass} />
    </div>
  );
}
