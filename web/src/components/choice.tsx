/*
 * One thing to choose, drawn the same way wherever it is offered.
 *
 * The bar at the top and the settings screen each had their own, and the
 * settings screen had three: one for a plain list, one for languages, one for
 * how subtitles are dressed. Four copies of the same label, the same wrapper
 * and the same classes, so that changing how a choice looks meant finding all
 * four and remembering the fourth.
 *
 * What differs between them is only where the wording of each option comes
 * from, so that is what is handed in. A number is a different shape and keeps
 * its own, sharing the wrapper.
 */

import type { ReactNode } from "react";
import { insideTheRange } from "../readable";

/** The wrapper both shapes share: a label, and the thing being chosen. */
function Wrapper({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="choice">
      <span className="choice-label">{label}</span>
      {children}
    </label>
  );
}

/**
 * A choice out of a list, each option with the wording it is shown under.
 *
 * The value is what the caller holds, which may be nothing at all: an empty
 * option stands for "no preference", and the caller says how it reads.
 */
export function Choice<T extends string>({
  label,
  value,
  options,
  onPick,
}: {
  label: string;
  value: T | "";
  /** Each option as it is kept, and as it is read. */
  options: [T | "", string][];
  onPick: (value: T) => void;
}) {
  return (
    <Wrapper label={label}>
      <select value={value} onChange={(event) => onPick(event.target.value as T)}>
        {options.map(([option, wording]) => (
          <option key={option} value={option}>
            {wording}
          </option>
        ))}
      </select>
    </Wrapper>
  );
}

/**
 * One number of a setting, with the range the server will keep it inside.
 *
 * The bounds are on the field as well as on the server, so somebody dragging
 * the arrows is stopped where the server would have stopped them rather than
 * being silently corrected afterwards.
 */
export function NumberChoice({
  label,
  value,
  min,
  max,
  disabled,
  onPick,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  disabled?: boolean;
  onPick: (value: number) => void;
}) {
  return (
    <Wrapper label={label}>
      <input
        type="number"
        className="choice-number"
        value={value}
        min={min}
        max={max}
        disabled={disabled}
        onChange={(event) => {
          const asked = insideTheRange(Number(event.target.value), min, max);
          if (asked !== null) {
            onPick(asked);
          }
        }}
      />
    </Wrapper>
  );
}
