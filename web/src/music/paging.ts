/*
 * A list of a library of music, read a page at a time as it is scrolled.
 *
 * A collection of a hundred thousand songs is never read whole: the first
 * page comes with the screen, the next ones as the end of what is drawn comes
 * near, and a letter jumped to reads on only as far as it needs.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import type { Page } from "./api";

/** How much one request brings: the most the server gives at once. */
export const PAGE = 200;

export interface Paged<T> {
  items: T[];
  /** How long the whole list is, once the first page has said. */
  total: number | null;
  loading: boolean;
  failed: boolean;
  /** Whether there is more to read after what is held. */
  more: boolean;
  /** Reads the next page, unless one is on its way or there is none. */
  loadMore: () => void;
  /** Reads on until the entry at this place is held. Answers whether it is. */
  reach: (index: number) => Promise<boolean>;
}

/**
 * Reads a list, starting again from its first page whenever `key` changes:
 * the key says what the list is (which library, in which order, narrowed to
 * what), so a list read another way is never mixed with the one before.
 */
export function usePaged<T>(
  key: string,
  read: (offset: number, limit: number, signal: AbortSignal) => Promise<Page<T>>,
): Paged<T> {
  const [items, setItems] = useState<T[]>([]);
  const [total, setTotal] = useState<number | null>(null);
  const [loading, setLoading] = useState(false);
  const [failed, setFailed] = useState(false);
  /* What is held, read without waiting for the page to be drawn again: two
     requests for more in one moment must not both ask for the same page. */
  const held = useRef<{ key: string; items: T[]; total: number | null; asking: Promise<void> | null }>({
    key,
    items: [],
    total: null,
    asking: null,
  });
  const reading = useRef(read);
  reading.current = read;
  const stop = useRef<AbortController | null>(null);

  const next = useCallback((): Promise<void> => {
    const now = held.current;
    if (now.asking) {
      return now.asking;
    }
    if (now.total !== null && now.items.length >= now.total) {
      return Promise.resolve();
    }
    const controller = stop.current ?? new AbortController();
    stop.current = controller;
    setLoading(true);
    const asking = reading
      .current(now.items.length, PAGE, controller.signal)
      .then((page) => {
        if (held.current !== now) {
          return;
        }
        now.items = [...now.items, ...page.items];
        now.total = page.total;
        setItems(now.items);
        setTotal(page.total);
        setFailed(false);
      })
      .catch((error: unknown) => {
        if (held.current === now && !(error instanceof DOMException && error.name === "AbortError")) {
          setFailed(true);
        }
      })
      .finally(() => {
        if (held.current === now) {
          now.asking = null;
          setLoading(false);
        }
      });
    now.asking = asking;
    return asking;
  }, []);

  useEffect(() => {
    stop.current?.abort();
    stop.current = new AbortController();
    held.current = { key, items: [], total: null, asking: null };
    setItems([]);
    setTotal(null);
    setFailed(false);
    void next();
    return () => stop.current?.abort();
  }, [key, next]);

  const reach = useCallback(
    async (index: number) => {
      const now = held.current;
      while (held.current === now && now.items.length <= index) {
        const before = now.items.length;
        await next();
        if (now.items.length === before) {
          break;
        }
      }
      return held.current === now && now.items.length > index;
    },
    [next],
  );

  return {
    items,
    total,
    loading,
    failed,
    more: total === null || items.length < total,
    loadMore: () => void next(),
    reach,
  };
}
