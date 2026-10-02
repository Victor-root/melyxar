/*
 * Pictures read and decoded before they are shown, and not before they are
 * about to be.
 *
 * A picture is decoded the moment it is first drawn, on the thread that draws,
 * and a big one stalls the frame it arrives in: the sheets of thumbnails are
 * thousands of points across. And a picture a page names is asked for as the
 * page opens, wherever on it it stands, which on a connection that carries six
 * at a time sends the little pictures at the top of the page to the back of the
 * queue behind sheets nobody can see yet.
 *
 * So the box that will show them says when they may be asked for: once it comes
 * within reach of the scroll, they are read and decoded off to the side, and
 * only then are they put on the page. The frame they arrive in costs what any
 * other does.
 */

import { useEffect, useState } from "react";
import type { RefObject } from "react";
import { scrollerOf } from "./landing";

/** How far ahead of the box, in heights of the window, they are asked for. */
const AHEAD = "150% 0px";

/** How many are read at once. A connection carries a handful at a time, and
 *  the pictures of the page in view must not wait behind a dozen sheets. */
const AT_ONCE = 2;

/** The pictures decoded so far, which may be put on the page, as they come:
 *  asked for in the order given, a couple at a time, once the box is within
 *  reach. Once one is, it stays so. */
export function useDecodedAhead(box: RefObject<Element | null>, urls: string[]): ReadonlySet<string> {
  const [ready, setReady] = useState<ReadonlySet<string>>(new Set());
  const wanted = urls.join("|");

  useEffect(() => {
    setReady(new Set());
    const element = box.current;
    if (!element || wanted === "") {
      return;
    }
    // Reach is measured in the box the page scrolls in: the margin of an
    // observer does not reach past a box that scrolls on its own.
    const scroller = element instanceof HTMLElement ? scrollerOf(element) : null;
    let gone = false;
    // Kept while they are being decoded, so that nothing lets go of them early.
    const held = new Set<HTMLImageElement>();
    const queue = wanted.split("|");

    const work = async (): Promise<void> => {
      for (let url = queue.shift(); url !== undefined && !gone; url = queue.shift()) {
        const image = new Image();
        image.decoding = "async";
        // Asked for after what is already on the page: they are for what is
        // coming, and the pictures in view are for now.
        image.fetchPriority = "low";
        image.src = url;
        held.add(image);
        await image.decode().catch(() => {});
        if (!gone) {
          const done = url;
          setReady((were) => new Set(were).add(done));
        }
      }
    };

    const watcher = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          watcher.disconnect();
          for (let worker = 0; worker < AT_ONCE; worker += 1) {
            void work();
          }
        }
      },
      { root: scroller, rootMargin: AHEAD },
    );
    watcher.observe(element);
    return () => {
      gone = true;
      watcher.disconnect();
      held.clear();
    };
  }, [box, wanted]);

  return ready;
}
