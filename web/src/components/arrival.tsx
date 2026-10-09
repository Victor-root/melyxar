/*
 * The first arrival on a page that is heavy to draw: held behind a quiet
 * loader until it is ready to be scrolled, then shown whole.
 *
 * Shown as soon as its answer came, a page full of pictures was drawn while
 * those pictures were still arriving and being decoded, and a scroll begun
 * at once met every one of them on its way: the stutter of a first scroll
 * that was gone a few seconds later. So the page is built out of sight, and
 * shown once the pictures of its first two screens are decoded.
 *
 * Held once per visit of the interface, never on a page walked back to,
 * whose pictures are already there, and never for longer than a few seconds:
 * a slow disk or network shows the page as it is rather than a loader that
 * does not end.
 */

import { useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { useBranding } from "../player/logo";
import { useSettings } from "../settings";
import { ServerMark } from "./server-mark";

/** The longest a page is held, from the moment the loader first shows. */
const LONGEST_HOLD_MS = 3000;

/** How far down the page the pictures waited for go, in screens. */
const SCREENS_WAITED_FOR = 2;

/** Whether a page has already arrived in this visit, and since when the one
 *  arriving has been held. Kept outside any page: the loader is drawn before
 *  the page's answer and the page after it, as two different trees. */
let arrived = false;
let heldSince: number | null = null;

/** Whether the page about to be drawn is still to be held. */
export function stillArriving(): boolean {
  return !arrived;
}

/** The loader alone, while there is not yet anything to build. */
export function ArrivalLoader() {
  const { t } = useSettings();
  const branding = useBranding();
  heldSince ??= performance.now();
  return (
    <div className="arrival" role="status" aria-label={t("arrival.loading")}>
      <ServerMark branding={branding} size={56} logoClassName="arrival-logo" />
      <span className="arrival-ring" aria-hidden="true" />
    </div>
  );
}

/** The page, built out of sight under the loader until it is ready. */
export function Arrival({ children }: { children: ReactNode }) {
  const [held, setHeld] = useState(!arrived);
  const box = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!held) {
      return;
    }
    heldSince ??= performance.now();
    let gone = false;
    const show = () => {
      if (!gone) {
        arrived = true;
        heldSince = null;
        setHeld(false);
      }
    };

    // Read once the page is laid out, which is a frame after it is drawn.
    const frame = requestAnimationFrame(() => {
      const reach = window.innerHeight * SCREENS_WAITED_FOR;
      /* Only the pictures that are drawn: one inside a box the stylesheet
         hides, such as the wide picture of a tile that a desktop does not
         show, is never fetched and so never decoded, and waiting for it
         held the loader for the whole of the longest hold, every time. */
      const pictures = Array.from(box.current?.querySelectorAll("img") ?? []).filter(
        (picture) => picture.getClientRects().length > 0 && picture.getBoundingClientRect().top < reach,
      );
      void Promise.allSettled(pictures.map((picture) => picture.decode())).then(show);
    });
    const late = setTimeout(show, Math.max(0, LONGEST_HOLD_MS - (performance.now() - (heldSince ?? 0))));

    return () => {
      gone = true;
      cancelAnimationFrame(frame);
      clearTimeout(late);
    };
  }, [held]);

  return (
    <>
      {held && <ArrivalLoader />}
      <div ref={box} className={held ? "arrival-held" : "arrival-shown"} aria-busy={held}>
        {children}
      </div>
    </>
  );
}
