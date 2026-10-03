/*
 * What a notification says, in the language being read, and whether it may
 * show on the screen right now. Kept apart from what draws it, so both can
 * be tested without a page.
 */

import { howMany, readableDate } from "../readable";
import type { Wording } from "../readable";
import type { Note, NoteSettings } from "./api";

/** How many titles are named before the rest are counted. */
const NAMED_AT_MOST = 3;

const MINUTES_IN_A_DAY = 24 * 60;

/** A notification put into words, and where pressing it leads. */
export interface NoteSaid {
  title: string;
  detail: string | null;
  to: string | null;
}

/** A few titles named, the rest counted. */
function someOf(titles: string[], total: number, t: Wording): string {
  const named = titles.slice(0, NAMED_AT_MOST).join(", ");
  const rest = total - Math.min(titles.length, NAMED_AT_MOST);
  return rest > 0 ? t("notes.and_more", { named, count: rest }) : named;
}

export function sayNote(note: Note, t: Wording, language: string): NoteSaid {
  const said = note.data;
  const work = note.work_id ? `/work/${note.work_id}` : null;
  switch (said?.kind) {
    case "message":
      return { title: said.title, detail: said.text || null, to: null };
    case "maintenance": {
      const when = note.due_at ? readableDate(note.due_at, language) : null;
      const at = when ? t("notes.maintenance_at", { when }) : null;
      return { title: said.title, detail: [at, said.text].filter(Boolean).join(" · ") || null, to: null };
    }
    case "new_content": {
      const library = `/library/${said.library}`;
      const episodes = said.series.reduce((sum, series) => sum + series.episodes, 0);
      if (said.series.length === 0 && said.films === 1) {
        return { title: t("notes.new_film", { title: said.film_titles[0] ?? "" }), detail: said.library_name, to: work };
      }
      if (said.series.length === 0) {
        return {
          title: t("notes.new_films", { count: said.films }),
          detail: someOf(said.film_titles, said.films, t),
          to: library,
        };
      }
      if (said.films === 0 && said.series.length === 1) {
        const series = said.series[0];
        return {
          title:
            series.episodes === 1
              ? t("notes.new_episode", { series: series.title })
              : t("notes.new_episodes", { count: series.episodes, series: series.title }),
          detail: said.library_name,
          to: `/work/${series.id}`,
        };
      }
      return {
        title: t("notes.new_many", { count: said.films + episodes, library: said.library_name }),
        detail: someOf(
          [...said.series.map((series) => series.title), ...said.film_titles],
          said.series.length + said.films,
          t,
        ),
        to: library,
      };
    }
    case "deletion":
      return said.checked_gone === null
        ? { title: t("delete.done_library"), detail: t("delete.done_library_why"), to: null }
        : {
            title: t("delete.done_disk"),
            detail:
              said.checked_gone === 0
                ? t("delete.done_disk_none")
                : howMany(said.checked_gone, "delete.done_disk_checked", t),
            to: null,
          };
    default:
      return { title: t("notes.unreadable"), detail: null, to: null };
  }
}

/** Whether a minute of the day falls in quiet hours, which may run across
 *  midnight. */
export function isQuiet(minute: number, settings: NoteSettings): boolean {
  const { quiet_from: from, quiet_until: until } = settings;
  if (from === null || until === null) {
    return false;
  }
  const now = ((minute % MINUTES_IN_A_DAY) + MINUTES_IN_A_DAY) % MINUTES_IN_A_DAY;
  return from < until ? now >= from && now < until : now >= from || now < until;
}

/** The minute of the day on this device's clock. */
export function minuteOfTheDay(now: Date): number {
  return now.getHours() * 60 + now.getMinutes();
}

/**
 * Whether a notification that wants the screen may have it now. During a
 * film, and during quiet hours, only what is urgent shows, and only for an
 * account that said it should.
 */
export function showsNow(
  note: Pick<Note, "priority" | "mandatory">,
  settings: NoteSettings,
  filmOnScreen: boolean,
  minute: number,
): boolean {
  const urgent = note.priority || note.mandatory;
  if (filmOnScreen && !(urgent && settings.priority_while_playing)) {
    return false;
  }
  if (isQuiet(minute, settings) && !(urgent && settings.priority_while_quiet)) {
    return false;
  }
  return true;
}

/** A minute of the day as a clock field writes it. */
export function clockOf(minute: number): string {
  const hours = Math.floor(minute / 60);
  return `${String(hours).padStart(2, "0")}:${String(minute % 60).padStart(2, "0")}`;
}

/** What a clock field holds, as a minute of the day; nothing when it holds
 *  no time. */
export function minuteOfClock(clock: string): number | null {
  const read = /^(\d{1,2}):(\d{2})$/.exec(clock);
  if (!read) {
    return null;
  }
  const [hours, minutes] = [Number(read[1]), Number(read[2])];
  return hours < 24 && minutes < 60 ? hours * 60 + minutes : null;
}
