/*
 * Two rows that share a line, with a hairline between them while they do:
 * what is already under way on the home page, and the cast beside the guest
 * stars of an episode.
 *
 * Whether they do is the stylesheet's answer rather than this file's: it
 * lays them side by side until the line runs out, on the width of the
 * window it is being read in. What is read back here is only that answer,
 * and only so a hairline can be drawn between them while they are on one
 * line. Down the side of a row that is on a line of its own, the same
 * hairline would be a rule drawn between nothing and nothing.
 */

import { useLayoutEffect, useRef, useState } from "react";

export function Together({
  children,
  oneLine = false,
}: {
  children: React.ReactNode;
  /** Kept on one line however long they are, each scrolling in its share
   *  of it, rather than put one under the other once they no longer fit. */
  oneLine?: boolean;
}) {
  const box = useRef<HTMLDivElement>(null);
  const [sharing, setSharing] = useState(false);

  useLayoutEffect(() => {
    const it = box.current;
    if (!it) {
      return;
    }
    const look = () => {
      const rows = Array.from(it.children) as HTMLElement[];
      setSharing(rows.length === 2 && rows[0].offsetTop === rows[1].offsetTop);
    };
    look();

    /* Watched rather than worked out: what decides this is the width the
       rows are given and the width of what they hold, and both change
       without this page being drawn again. */
    const watch = new ResizeObserver(look);
    watch.observe(it);
    for (const row of Array.from(it.children)) {
      watch.observe(row);
    }
    return () => watch.disconnect();
  }, [children]);

  return (
    <div className={`rows-together${oneLine ? " rows-one-line" : ""}${sharing ? " rows-sharing" : ""}`} ref={box}>
      {children}
    </div>
  );
}
