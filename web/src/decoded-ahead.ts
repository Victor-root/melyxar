/*
 * Pictures read and decoded before they are on the screen.
 *
 * A picture is decoded the moment it is first drawn, on the thread that draws,
 * and a big one stalls the frame it arrives in: the sheets of thumbnails are
 * thousands of points across. Asked for in advance, as the box that will show
 * them comes within reach of the scroll, the browser decodes them off to the
 * side and the frame they arrive in costs what any other does.
 */

import { useEffect } from "react";
import type { RefObject } from "react";

/** How far ahead of the box, in heights of the window, they are asked for. */
const AHEAD = "150% 0px";

export function useDecodedAhead(box: RefObject<Element | null>, urls: string[]) {
  const wanted = urls.join("|");
  useEffect(() => {
    const element = box.current;
    if (!element || wanted === "") {
      return;
    }
    // Held, not just asked for, while the box is within reach, and let go
    // once it is not: what is kept is only what is about to be seen.
    const held = new Set<HTMLImageElement>();
    const watcher = new IntersectionObserver(
      ([entry]) => {
        held.clear();
        if (!entry.isIntersecting) {
          return;
        }
        for (const url of wanted.split("|")) {
          const image = new Image();
          image.decoding = "async";
          image.src = url;
          image.decode().catch(() => {});
          held.add(image);
        }
      },
      { rootMargin: AHEAD },
    );
    watcher.observe(element);
    return () => {
      watcher.disconnect();
      held.clear();
    };
  }, [box, wanted]);
}
