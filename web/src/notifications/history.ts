/*
 * The part of an account's history a page holds, and how each change moves
 * it: which notifications are shown, and how many are unread over the whole
 * history, the pages not yet read included.
 */

import type { Note } from "./api";

export interface Held {
  /** Newest first. */
  notes: Note[];
  /** Unread over the whole history. */
  unread: number;
  /** Whether older pages remain to be read. */
  more: boolean;
}

export const NOTHING_HELD: Held = { notes: [], unread: 0, more: false };

/** Identifiers are ordered by time, written the same length: as text, the
 *  later is the greater. */
function newestFirst(notes: Note[]): Note[] {
  return [...notes].sort((one, other) => (one.id < other.id ? 1 : one.id > other.id ? -1 : 0));
}

/** Whether a notification belongs among those held, rather than in a page
 *  not read yet. */
function withinHeld(held: Held, id: string): boolean {
  const oldest = held.notes[held.notes.length - 1];
  return !held.more || oldest === undefined || id > oldest.id;
}

/** A notification that arrived, or came back unread. */
export function arrived(held: Held, note: Note): Held {
  const there = held.notes.find((one) => one.id === note.id);
  const becameUnread = !note.read && (there === undefined || there.read) ? 1 : 0;
  const others = held.notes.filter((one) => one.id !== note.id);
  return {
    ...held,
    notes: there !== undefined || withinHeld(held, note.id) ? newestFirst([note, ...others]) : held.notes,
    unread: held.unread + becameUnread,
  };
}

/** Marks notifications read or unread. One not held is counted all the same:
 *  the server only says what changed. */
export function marked(held: Held, ids: string[], read: boolean): Held {
  let unread = held.unread;
  const changing = new Set(ids);
  for (const id of ids) {
    const there = held.notes.find((one) => one.id === id);
    if (there === undefined || there.read !== read) {
      unread += read ? -1 : 1;
    }
  }
  return {
    ...held,
    notes: held.notes.map((one) => (changing.has(one.id) ? { ...one, read } : one)),
    unread: Math.max(0, unread),
  };
}

/** Takes notifications away. Answers whether one of them was not held, in
 *  which case nobody here knows whether it was unread. */
export function removed(held: Held, ids: string[]): { held: Held; unknown: boolean } {
  const going = held.notes.filter((one) => ids.includes(one.id));
  return {
    held: {
      ...held,
      notes: held.notes.filter((one) => !ids.includes(one.id)),
      unread: Math.max(0, held.unread - going.filter((one) => !one.read).length),
    },
    unknown: going.length < ids.length,
  };
}

/** The ids among these that are held and unread. */
export function unreadAmong(held: Held, ids?: string[]): string[] {
  return held.notes.filter((one) => !one.read && (ids === undefined || ids.includes(one.id))).map((one) => one.id);
}

/** An older page read, put after what is held. */
export function olderPage(held: Held, page: { notifications: Note[]; more: boolean }): Held {
  const known = new Set(held.notes.map((one) => one.id));
  return {
    ...held,
    notes: [...held.notes, ...page.notifications.filter((one) => !known.has(one.id))],
    more: page.more,
  };
}
