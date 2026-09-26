/*
 * The bar down the right hand side of the library.
 *
 * Drawn here rather than left to the browser, for one reason: a scrollbar the
 * browser draws keeps a strip of the window to itself, and everything inside
 * the box that scrolls is cut off before it. A banner meant to run to both
 * edges of the screen therefore stops a bar's width short of the right one,
 * and nothing inside the box can ever reach past that, because the box is
 * clipped exactly where the bar begins. This one takes no strip: it stands
 * over the page on its own, so the picture runs the whole width and the bar
 * floats on it, which is the way round it was asked for.
 *
 * The scrolling itself is still the browser's, untouched. The wheel, the
 * keys, the trackpad and the touch of a finger all work as they did; what is
 * replaced is only the thing that shows where in the page somebody is, and
 * the two ways of moving by hand that come with it.
 *
 * Nothing here holds a position in React state. A bar redrawn through a
 * render on every frame of a scroll is a page doing the work of a scroll
 * twice, so the mark is moved by writing to it directly, once per frame at
 * most. The one thing that goes through state is whether there is anything to
 * scroll at all, which changes when a page does and not while one is read.
 */

import { useCallback, useEffect, useRef, useState } from "react";

/** The shortest the mark is ever drawn. A page of many screens would
 *  otherwise leave a few points of colour nobody can catch with a pointer. */
const SHORTEST = 38;

export function ScrollBar({ holder }: { holder: React.RefObject<HTMLElement | null> }) {
  const track = useRef<HTMLDivElement>(null);
  const mark = useRef<HTMLDivElement>(null);
  /* The length last written. How long the mark is changes when the page
     does and not while one is scrolled, and writing a height is what makes
     a browser work the layout out again: written every frame, the one thing
     here that costs anything would have been costing it sixty times a
     second for nothing. */
  const drawn = useRef(-1);
  /* Whether there is more page than screen. Drawn nowhere at all when there
     is not: a bar as long as its own track says nothing and is one more thing
     on a screen that does not scroll. */
  const [longer, setLonger] = useState(false);

  /** Where the mark goes and how long it is, from where the page stands. */
  const draw = useCallback(() => {
    const box = holder.current;
    const road = track.current;
    const it = mark.current;
    if (!box || !road) {
      return;
    }
    const view = box.clientHeight;
    const over = box.scrollHeight - view;
    /* A point of slack: a page a hair longer than its screen is a page
       nobody scrolls, and a bar for it is a bar that flickers in and out as
       a row of cards settles. */
    setLonger(over > 1);
    if (!it || over <= 1) {
      return;
    }
    const room = road.clientHeight;
    const long = Math.max(SHORTEST, (view / box.scrollHeight) * room);
    if (long !== drawn.current) {
      drawn.current = long;
      it.style.height = `${long}px`;
    }
    it.style.transform = `translateY(${(box.scrollTop / over) * (room - long)}px)`;
  }, [holder]);

  /*
   * What the mark is redrawn for: the page being scrolled, the window being
   * resized, and the page itself growing or shrinking, which is a scan
   * filling a row, a picture arriving, or another screen being opened.
   *
   * The parts watched are the box and whatever it holds at the time, and the
   * list is taken again whenever the box is handed something else, which is
   * what going from one screen to another does.
   *
   * A change of size is drawn straight from the observer, which the browser
   * calls once the page has been laid out: the sizes read there are already
   * known. Read from a frame of its own instead, they made the browser lay
   * out the whole page ahead of time as a screen opened, forty milliseconds
   * of it on a laptop, and then again once the rest of the frame had its
   * turn. Watching a part starts with a call for it, which is what draws the
   * mark the first time and each time the box holds something new.
   */
  useEffect(() => {
    const box = holder.current;
    if (!box) {
      return;
    }
    let asked = 0;
    const soon = () => {
      if (!asked) {
        asked = requestAnimationFrame(() => {
          asked = 0;
          draw();
        });
      }
    };

    const sizes = new ResizeObserver(draw);
    const watch = () => {
      sizes.disconnect();
      sizes.observe(box);
      for (const part of Array.from(box.children)) {
        sizes.observe(part);
      }
    };
    watch();
    const swapped = new MutationObserver(watch);
    swapped.observe(box, { childList: true });
    box.addEventListener("scroll", soon, { passive: true });

    return () => {
      box.removeEventListener("scroll", soon);
      sizes.disconnect();
      swapped.disconnect();
      if (asked) {
        cancelAnimationFrame(asked);
      }
    };
  }, [holder, draw]);

  /* Dragging the mark. The page follows the pointer for as long as it is
     held, wherever it goes: a drag that stops the moment the pointer leaves
     the bar is a drag that lets go in the middle of a long page. */
  const take = (event: React.PointerEvent<HTMLDivElement>) => {
    const box = holder.current;
    const road = track.current;
    const it = mark.current;
    if (!box || !road || !it) {
      return;
    }
    event.preventDefault();
    const held = event.clientY - it.getBoundingClientRect().top;

    const follow = (moved: PointerEvent) => {
      const room = road.clientHeight - it.offsetHeight;
      if (room <= 0) {
        return;
      }
      const down = moved.clientY - road.getBoundingClientRect().top - held;
      box.scrollTop =
        (Math.min(Math.max(down, 0), room) / room) * (box.scrollHeight - box.clientHeight);
    };
    const letGo = () => {
      window.removeEventListener("pointermove", follow);
      window.removeEventListener("pointerup", letGo);
      window.removeEventListener("pointercancel", letGo);
    };
    window.addEventListener("pointermove", follow);
    window.addEventListener("pointerup", letGo);
    window.addEventListener("pointercancel", letGo);
  };

  /* A press on the track itself, above or below the mark: one screen that
     way, which is what the page up and page down keys do and what every
     scrollbar has done since there were scrollbars. */
  const step = (event: React.PointerEvent<HTMLDivElement>) => {
    const box = holder.current;
    const it = mark.current;
    if (!box || !it || event.target !== event.currentTarget) {
      return;
    }
    const where = it.getBoundingClientRect();
    box.scrollBy({
      top: event.clientY < where.top ? -box.clientHeight : box.clientHeight,
      behavior: "smooth",
    });
  };

  return (
    <div
      className={`scrollbar${longer ? " scrollbar-long" : ""}`}
      ref={track}
      onPointerDown={step}
      aria-hidden="true"
    >
      <div className="scrollbar-mark" ref={mark} onPointerDown={take} />
    </div>
  );
}
