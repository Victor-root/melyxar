/*
 * How a value the server sends is said to somebody.
 *
 * Rates, sizes, instants, times of day, fractions. None of it is drawing: a
 * screen laid out again from nothing still has to turn eight million bits a
 * second into something a person reads, and still has to say the server's
 * clock in the reader's hour.
 *
 * Together in one place because they were not, and the cost of that was
 * visible: the same instant was turned into local time in three different
 * ways on three different screens, and a fraction was rounded one way in a
 * list and another way in the bar above it.
 */

const MINUTES_IN_A_DAY = 24 * 60;

function wrapIntoADay(minutes: number): number {
  return ((minutes % MINUTES_IN_A_DAY) + MINUTES_IN_A_DAY) % MINUTES_IN_A_DAY;
}

/**
 * An instant the server wrote, in the hour of whoever is reading it.
 *
 * The server keeps one clock and it is UTC, which is the only one it can read
 * with certainty. Three in the morning there is four here half the year, and
 * announcing the server's hour to somebody looking at their own clock is how a
 * run that happened on time looks like a run that did not.
 */
export function whenItIs(instant: string | null): string {
  if (!instant) {
    return "";
  }
  const when = new Date(instant);
  return Number.isNaN(when.getTime()) ? instant : when.toLocaleString();
}

/** The same, spelled out, in whichever language the interface is speaking. */
export function readableDate(value: string, language: string): string | null {
  const when = new Date(value);
  return Number.isNaN(when.getTime())
    ? null
    : when.toLocaleString(language, { dateStyle: "medium", timeStyle: "short" });
}

/**
 * The time of day an instant carries, and nothing else.
 *
 * What a journal read while a test is running wants: the day is the day the
 * reader is living through.
 */
export function timeOfDay(instant: string): string {
  return instant.slice(11, 19);
}

/**
 * A time of day the server keeps in universal time, in the time of this
 * browser.
 *
 * Nobody should have to do that conversion in their head, so it is done here,
 * where the browser knows its own offset. What this cannot do is follow the
 * clocks changing: a time set in winter shows an hour later in summer until
 * somebody sets it again. That is said on the screen rather than hidden, and
 * it is a nightly piece of upkeep, so an hour either way costs nothing.
 */
export function asLocalTime(utcMinutes: number): string {
  const local = wrapIntoADay(utcMinutes - new Date().getTimezoneOffset());
  const hours = String(Math.floor(local / 60)).padStart(2, "0");
  const minutes = String(local % 60).padStart(2, "0");
  return `${hours}:${minutes}`;
}

/** The other way, for what a time field hands back. */
export function asUtcMinutes(localTime: string): number {
  const [hours, minutes] = localTime.split(":").map(Number);
  if (!Number.isFinite(hours) || !Number.isFinite(minutes)) {
    return 0;
  }
  return wrapIntoADay(hours * 60 + minutes + new Date().getTimezoneOffset());
}

/**
 * How far along, rounded down until it really is finished.
 *
 * Rounded the usual way, a pass on its last film out of three hundred and
 * forty seven reads a hundred per cent while it still has a whole film to
 * read: seen on the scan, where that last film was several minutes. A hundred
 * per cent is said when it is a hundred per cent.
 */
export function outOfAHundred(ratio: number): number {
  return ratio >= 1 ? 100 : Math.min(99, Math.floor(ratio * 100));
}

/** What a film is rated, on the scale the provider uses. */
export function outOfTen(rating: number): string {
  return `${rating.toFixed(1)} / 10`;
}

/** A rate, in the unit that keeps it to a few figures. */
export function readableBitrate(bits: number | null): string | null {
  if (bits === null || bits <= 0) {
    return null;
  }
  return bits >= 1_000_000
    ? `${(bits / 1_000_000).toFixed(1)} Mb/s`
    : `${Math.round(bits / 1000)} kb/s`;
}

/** A size, in the unit that keeps it to a few figures. */
export function readableSize(bytes: number): string {
  const units = ["B", "kB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1000 && unit < units.length - 1) {
    value /= 1000;
    unit += 1;
  }
  return unit === 0 ? `${value} ${units[unit]}` : `${value.toFixed(1)} ${units[unit]}`;
}

/**
 * A number a field handed back, brought inside what the server will keep.
 *
 * The bounds are here as well as on the server, so somebody dragging the
 * arrows is stopped where the server would have stopped them rather than
 * being silently corrected afterwards.
 */
export function insideTheRange(asked: number, min: number, max: number): number | null {
  if (!Number.isFinite(asked)) {
    return null;
  }
  return Math.min(max, Math.max(min, Math.round(asked)));
}
