/*
 * The bubble of a phone: three dots in the band at the top, rolled out by a
 * press to the width of the band and rolled up again by themselves once
 * nobody has touched them. What is in it, the tabs of a library of music or
 * what a list of films is read by, is the caller's; this is only how it rolls.
 */

import { useCallback, useEffect, useRef, useState } from "react";

/** How long the bubble takes to roll out, as the stylesheet draws it. */
export const UNROLL_MS = 340;

/** How long it stays rolled out once nobody has touched it. */
const UNROLLED_MS = 2000;

export function useBubble() {
  const [unrolled, setUnrolled] = useState(false);
  const rollUp = useRef(0);
  const bubble = useRef<HTMLElement>(null);

  /* A list of choices open from inside it keeps it rolled out: it would
     leave the list standing over a field that is no longer there. */
  const stay = useCallback(() => {
    window.clearTimeout(rollUp.current);
    rollUp.current = window.setTimeout(function roll() {
      if (bubble.current?.querySelector(".picker-open")) {
        rollUp.current = window.setTimeout(roll, UNROLLED_MS);
        return;
      }
      setUnrolled(false);
    }, UNROLLED_MS);
  }, []);

  const unroll = useCallback(() => {
    setUnrolled(true);
    stay();
  }, [stay]);

  useEffect(() => () => window.clearTimeout(rollUp.current), []);

  /* A press anywhere else rolls it up at once, the lists it opens being part
     of it. */
  useEffect(() => {
    if (!unrolled) {
      return;
    }
    const away = (event: PointerEvent) => {
      const on = event.target as Element;
      if (!bubble.current?.contains(on) && !on.closest(".picker-list")) {
        setUnrolled(false);
      }
    };
    document.addEventListener("pointerdown", away, true);
    return () => document.removeEventListener("pointerdown", away, true);
  }, [unrolled]);

  return { unrolled, setUnrolled, unroll, stay, bubble };
}

/** The glass of the bubble, its line and the dots that roll it out. */
export function BubbleParts({
  unrolled,
  onUnroll,
  label,
}: {
  unrolled: boolean;
  onUnroll: () => void;
  label: string;
}) {
  return (
    <>
      <span className="glass-slice music-tabs-glass" aria-hidden="true" />
      <span className="glass-rim music-tabs-rim" aria-hidden="true" />
      <span className="music-tabs-end music-tabs-end-start" aria-hidden="true">
        <span className="glass-rim" />
      </span>
      <span className="music-tabs-end music-tabs-end-end" aria-hidden="true">
        <span className="glass-rim" />
      </span>
      <button
        type="button"
        className="music-bubble"
        aria-label={label}
        aria-expanded={unrolled}
        onClick={onUnroll}
      >
        <svg viewBox="0 0 24 24" width="22" height="22" fill="currentColor" aria-hidden="true">
          <circle cx="5" cy="12" r="2.6" />
          <circle cx="12" cy="12" r="2.6" />
          <circle cx="19" cy="12" r="2.6" />
        </svg>
      </button>
    </>
  );
}
