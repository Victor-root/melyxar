/*
 * A list of a library of music, read a page at a time as it is scrolled.
 *
 * A collection of a hundred thousand songs is never read whole: the first
 * page comes with the screen, the next ones as the end of what is drawn comes
 * near, and a letter jumped to reads on only as far as it needs.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { keep, recall } from "../kept";
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

/** What a list held when it was left, kept for when it is walked back to. */
interface Kept<T> {
  items: T[];
  total: number | null;
}

const keptAs = (key: string) => `music-list|${key}`;

/**
 * Reads a list, starting again from its first page whenever `key` changes:
 * the key says what the list is (which library, in which order, narrowed to
 * what), so a list read another way is never mixed with the one before.
 *
 * When `version` moves, the library changed under the list: what is held is
 * read again and put in its place at once, the list neither emptied nor
 * moved, so an album filed or a cover found during a scan simply appears.
 *
 * A list walked back to is drawn whole at once from what it held when it was
 * left, and read again quietly: the page can then be put back where it was
 * scrolled to, which an empty list could not be.
 */
export function usePaged<T>(
  key: string,
  read: (offset: number, limit: number, signal: AbortSignal) => Promise<Page<T>>,
  version: number | undefined,
): Paged<T> {
  const [items, setItems] = useState<T[]>(() => recall<Kept<T>>(keptAs(key))?.value.items ?? []);
  const [total, setTotal] = useState<number | null>(() => recall<Kept<T>>(keptAs(key))?.value.total ?? null);
  const [loading, setLoading] = useState(false);
  const [failed, setFailed] = useState(false);
  /* What is held, read without waiting for the page to be drawn again: two
     requests for more in one moment must not both ask for the same page. */
  const held = useRef<{ key: string; items: T[]; total: number | null; asking: Promise<void> | null }>({
    key,
    items,
    total,
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

  /* What is held, read again from the start and put in its place at once,
     the list neither emptied nor moved. */
  const readAgain = useCallback((signal: AbortSignal) => {
    const now = held.current;
    const wanted = Math.max(now.items.length, PAGE);
    void (async () => {
      const again: T[] = [];
      let whole: number | null = null;
      for (let offset = 0; offset < wanted; offset += PAGE) {
        const page = await reading.current(offset, PAGE, signal);
        again.push(...page.items);
        whole = page.total;
        if (page.items.length < PAGE) {
          break;
        }
      }
      if (held.current === now && !now.asking) {
        now.items = again;
        now.total = whole;
        setItems(again);
        setTotal(whole);
      }
    })().catch(() => {});
  }, []);

  useEffect(() => {
    stop.current?.abort();
    const controller = new AbortController();
    stop.current = controller;
    const kept = recall<Kept<T>>(keptAs(key))?.value;
    held.current = { key, items: kept?.items ?? [], total: kept?.total ?? null, asking: null };
    setItems(held.current.items);
    setTotal(held.current.total);
    setFailed(false);
    if (kept && kept.items.length > 0) {
      readAgain(controller.signal);
    } else {
      void next();
    }
    return () => controller.abort();
  }, [key, next, readAgain]);

  useEffect(() => {
    if (total !== null && held.current.key === key && held.current.items === items) {
      keep(keptAs(key), { items, total } satisfies Kept<T>);
    }
  }, [key, items, total]);

  const seenVersion = useRef(version);
  useEffect(() => {
    if (version === seenVersion.current) {
      return;
    }
    seenVersion.current = version;
    const controller = new AbortController();
    readAgain(controller.signal);
    return () => controller.abort();
  }, [version, readAgain]);

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
