/*
 * What has been said about each work, held outside of React's own state so
 * that a card can listen to its own work and to no other.
 *
 * Held in React's state, one press on one card told every card of the page
 * that something had changed, and every one of them drew itself again to
 * find that it had not. Here each reader asks for the part it reads, and is
 * told only when that part is a different one.
 */

import type { Said } from "./watching";

/** What has been said about one work since the page was drawn. */
export interface Mark {
  watched?: Said;
  favourite?: boolean;
  /** Put aside to watch later, and what the server had said when it was.
      Seeing the work takes it off the list on the server's side, so a card
      that has since come back saying otherwise is the server's newer word. */
  watchLater?: { said: boolean; over: boolean };
  /** On the server's own front shelf. Unlike those above, nothing is known
      about this until somebody says it here: a card does not arrive saying
      whether it is on the front page. */
  pinned?: boolean;
  /** Deleted from here. Nothing draws it again: a card that stays in a grid
      after its work has gone is a card that fails when it is pressed. */
  gone?: boolean;
}

/** Everything said, by work. A new record at each change, and the entry of
 *  a work nobody touched is the very one it was. */
export type Marked = Readonly<Record<string, Mark>>;

export interface MarkStore {
  /** Everything said so far. */
  all: () => Marked;
  /** What was said about one work, the same object until it is said again. */
  of: (id: string) => Mark | undefined;
  /** Says more about one or several works, and tells whoever listens. */
  say: (changes: Record<string, Mark>) => void;
  /** Told after every change. Answers how to stop being told. */
  subscribe: (listener: () => void) => () => void;
}

export function createMarkStore(): MarkStore {
  let marked: Marked = {};
  const listeners = new Set<() => void>();

  return {
    all: () => marked,
    of: (id) => marked[id],
    say: (changes) => {
      const after = { ...marked };
      for (const [id, mark] of Object.entries(changes)) {
        after[id] = { ...marked[id], ...mark };
      }
      marked = after;
      for (const listener of listeners) {
        listener();
      }
    },
    subscribe: (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
  };
}
