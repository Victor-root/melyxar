/*
 * An hour of the day chosen in Melyxar's own lists, the hours in one and the
 * minutes in the other, rather than the browser's own time field, which
 * draws itself in the system's colours and in a shape of its own.
 */

import { Picker } from "./panel";

/** The minutes offered, five at a time. */
const MINUTE_STEP = 5;

const twoDigits = (n: number) => String(n).padStart(2, "0");

/** The hours of a day, written as a time field writes them. */
export const HOURS = Array.from({ length: 24 }, (_, hour) => twoDigits(hour));

/** The minutes offered for an hour: every five, and the one held when it is
 *  not one of them, so an hour set elsewhere is shown as it is. */
export function minutesOffered(held: string): string[] {
  const offered = Array.from({ length: 60 / MINUTE_STEP }, (_, step) => twoDigits(step * MINUTE_STEP));
  return held && !offered.includes(held) ? [...offered, held].sort() : offered;
}

/**
 * An hour of the day as "HH:MM". With `unset`, it may also be no hour at all,
 * an empty value, offered first in the list of hours under that wording.
 */
export function HourField({
  value,
  onPick,
  label,
  disabled,
  unset,
}: {
  value: string;
  onPick: (value: string) => void;
  label: string;
  disabled?: boolean;
  unset?: string;
}) {
  const [hours, minutes] = value ? value.split(":") : ["", ""];
  return (
    <span className="hour-field">
      <Picker
        label={label}
        value={hours}
        disabled={disabled}
        options={[
          ...(unset === undefined ? [] : [["", unset] as const]),
          ...HOURS.map((hour) => [hour, hour] as const),
        ]}
        onPick={(hour) => onPick(hour ? `${hour}:${minutes || "00"}` : "")}
      />
      <span aria-hidden="true">:</span>
      <Picker
        label={label}
        value={minutes}
        disabled={disabled || !hours}
        options={minutesOffered(minutes).map((minute) => [minute, minute] as const)}
        onPick={(minute) => onPick(`${hours}:${minute}`)}
      />
    </span>
  );
}
