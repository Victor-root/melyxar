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

/** How the interface says a thing, in whichever language it is speaking. */
export type Wording = (key: string, values?: Record<string, string | number>) => string;

/**
 * How a season or an episode is announced.
 *
 * Always by its number, because that is what somebody is looking for, and in
 * the language the page is being read in. The name written in the database is
 * only ever added to that: a scan that could read nothing but a number wrote
 * the number out in English, and the server says so by sending no name at all.
 */
export function nameOfOne(
  kind: string,
  number: number | null,
  title: string | null,
  t: Wording,
): string {
  const numbered = numberOfOne(kind, number, t);
  if (!numbered) {
    return title ?? "";
  }
  return title ? `${numbered} · ${title}` : numbered;
}

/** The number alone, for a row that shows the name in a column of its own. */
export function numberOfOne(kind: string, number: number | null, t: Wording): string {
  if (number === null) {
    return "";
  }
  if (kind === "season") {
    return number === SEASON_OF_SPECIALS ? t("work.specials") : t("work.season", { number });
  }
  return kind === "episode" ? t("work.episode", { number }) : "";
}

/** The season everything belonging to no season is filed under. */
const SEASON_OF_SPECIALS = 0;

/** How many of something, said with the wording that fits one of it. */
export function howMany(count: number, key: string, t: Wording): string {
  return count === 1 ? t(`${key}_one`) : t(key, { count });
}

const MINUTES_IN_A_DAY = 24 * 60;

function wrapIntoADay(minutes: number): number {
  return ((minutes % MINUTES_IN_A_DAY) + MINUTES_IN_A_DAY) % MINUTES_IN_A_DAY;
}

/**
 * The release a build belongs to, without the commit it was built from: what
 * a person compares with a list of versions. The whole of it is still said
 * where there is room, for whoever reports a problem.
 */
export function releaseOf(build: string): string {
  return build.split(" (")[0];
}

/**
 * How long ago an instant was, in the two largest units that say it: days and
 * hours, hours and minutes, or minutes alone. A server up for twelve days is
 * not up for seventeen thousand minutes, and the minutes of a twelve day run
 * are noise.
 */
export function howLongSince(instant: string, now: number, t: Wording): string {
  const minutes = Math.max(0, Math.floor((now - new Date(instant).getTime()) / 60_000));
  const days = Math.floor(minutes / MINUTES_IN_A_DAY);
  const hours = Math.floor((minutes % MINUTES_IN_A_DAY) / 60);
  if (days > 0) {
    return t("time.days_hours", { days, hours });
  }
  if (hours > 0) {
    return t("time.hours_minutes", { hours, minutes: minutes % 60 });
  }
  return t("time.minutes", { minutes });
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

/**
 * How long a film is, said the way anybody says it out loud.
 *
 * A hundred and sixty one minutes is a figure somebody has to divide in their
 * head before it means anything. Two hours forty one is the same number,
 * already read.
 */
export function howLong(minutes: number, t: Wording): string {
  if (minutes < 60) {
    return t("work.minutes", { count: minutes });
  }
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  return rest === 0
    ? t("work.hours", { hours })
    : t("work.hours_minutes", { hours, minutes: rest });
}

/**
 * Which episode a work is, when it is one.
 *
 * Short, because it is read under a still in a row and beside a title in a
 * banner rather than on a page of its own. The long form, "season one,
 * episode three", is what a menu and a detail page say.
 */
export function whichEpisode(
  work: { season_number: number | null; episode_number: number | null; title: string },
  t: Wording,
): string | undefined {
  if (work.season_number === null || work.episode_number === null) {
    return undefined;
  }
  const which = t("home.up_next.short", {
    season: work.season_number,
    episode: work.episode_number,
  });
  return `${which} · ${work.title}`;
}

/**
 * What the file itself holds, as the badges beside a title say it.
 *
 * The server sends what the file states and nothing more, because a height in
 * pixels is a fact and "4K" is a way of saying it. None of these are
 * translated: they are the names their own makers gave them, and a viewer
 * reading the interface in French is still looking for "Dolby Atmos".
 */
export function whatTheFileHolds(facts: {
  height: number | null;
  hdr: string | null;
  sound: string | null;
}): string[] {
  return [pictureName(facts.height), rangeName(facts.hdr), soundName(facts.sound)].filter(
    (name): name is string => name !== null,
  );
}

/** How tall a picture is, said as the badge on a box says it. */
function pictureName(height: number | null): string | null {
  if (height === null) {
    return null;
  }
  if (height >= 2000) {
    return "4K";
  }
  if (height >= 1000) {
    return "1080p";
  }
  return height >= 700 ? "720p" : null;
}

const RANGE_NAMES: Record<string, string> = {
  hdr10: "HDR10",
  hlg: "HLG",
  dolby_vision: "Dolby Vision",
};

function rangeName(hdr: string | null): string | null {
  return hdr === null ? null : (RANGE_NAMES[hdr] ?? hdr.toUpperCase());
}

/* A file states its codec in the short form an analyser uses. Nobody is
   looking for "eac3" on a badge. */
const SOUND_NAMES: Record<string, string> = {
  ac3: "Dolby Digital",
  eac3: "Dolby Digital+",
  truehd: "Dolby TrueHD",
  dts: "DTS",
  aac: "AAC",
  flac: "FLAC",
  opus: "Opus",
  mp3: "MP3",
  vorbis: "Vorbis",
};

function soundName(sound: string | null): string | null {
  if (sound === null) {
    return null;
  }
  // Atmos is written in the profile beside whatever carries it, and it is the
  // one thing somebody looks for when they have the speakers for it.
  if (/atmos/i.test(sound)) {
    return "Dolby Atmos";
  }
  if (sound.startsWith("pcm")) {
    return "PCM";
  }
  return SOUND_NAMES[sound.toLowerCase()] ?? sound;
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

/** The units of an amount of data, in each language the interface speaks. */
const DATA_UNITS: Record<string, string[]> = {
  en: ["B", "kB", "MB", "GB", "TB"],
  fr: ["o", "ko", "Mo", "Go", "To"],
};

/**
 * An amount of memory or disk in the units and with the decimal mark of the
 * language spoken: "6.2 GB" to one reader, "6,2 Go" to another.
 *
 * Counted by 1024, as Proxmox and Windows count, so a container given
 * 8192 MB reads 8 GB here as it does everywhere else its owner looks.
 */
export function amountOfData(bytes: number, language: string): string {
  const units = DATA_UNITS[language] ?? DATA_UNITS.en;
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = unit === 0 || value >= 100 ? 0 : 1;
  return `${value.toLocaleString(language, { maximumFractionDigits: digits })}\u00a0${units[unit]}`;
}

/**
 * How fast data passes over the network, in bits a second, which is how every
 * connection is sold and so the number anybody can compare with their own.
 */
export function networkRate(bytesPerSecond: number, language: string): string {
  const units = ["b/s", "kb/s", "Mb/s", "Gb/s"];
  let value = bytesPerSecond * 8;
  let unit = 0;
  while (value >= 1000 && unit < units.length - 1) {
    value /= 1000;
    unit += 1;
  }
  const digits = unit === 0 || value >= 100 ? 0 : 1;
  return `${value.toLocaleString(language, { maximumFractionDigits: digits, minimumFractionDigits: digits })}\u00a0${units[unit]}`;
}

/** A share from nought to one as a whole percentage, the way the language writes one. */
export function percentOf(share: number, language: string): string {
  return share.toLocaleString(language, { style: "percent", maximumFractionDigits: 0 });
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
