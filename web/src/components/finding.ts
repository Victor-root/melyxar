/*
 * The setting a search led to: its card brought into view, and pulsing
 * softly twice so the eye lands on it.
 *
 * The search says which one in the address, by the key it is named by. It is
 * looked for among the names of the settings and cards drawn on the page, and
 * waited for when the page is still loading what it shows.
 */

import { useEffect } from "react";
import type { RefObject } from "react";
import { useSearchParams } from "react-router-dom";
import { folded } from "../findable";
import { useSettings } from "../settings";

/** How long a page is given to draw the setting asked for. */
const WAITED_FOR_MS = 5000;

/** What a setting and a card are named by on a page. */
const NAMES = ".setting-label, .panel-head h2";

export function useFindOnArrival(page: RefObject<HTMLElement | null>): void {
  const { t } = useSettings();
  const [parameters, setParameters] = useSearchParams();
  const key = parameters.get("find");

  useEffect(() => {
    const holder = page.current;
    if (!key || !holder) {
      return;
    }
    const wanted = folded(t(key));
    const look = () => {
      const named = [...holder.querySelectorAll(NAMES)].find(
        (name) => folded(name.textContent ?? "").startsWith(wanted),
      );
      const card = named?.closest(".panel");
      if (!card) {
        return false;
      }
      card.scrollIntoView({ block: "center", behavior: "smooth" });
      // Taken off once the pulse is over, so it can be played again.
      card.addEventListener("animationend", () => card.classList.remove("found"), {
        once: true,
      });
      card.classList.add("found");
      return true;
    };
    // Asked for once, whatever happens: the address forgets it, so going back
    // to this page later does not light it again.
    const forget = () =>
      setParameters(
        (before) => {
          const after = new URLSearchParams(before);
          after.delete("find");
          return after;
        },
        { replace: true },
      );
    if (look()) {
      forget();
      return;
    }
    const drawn = new MutationObserver(() => {
      if (look()) {
        stop();
      }
    });
    const given = window.setTimeout(() => stop(), WAITED_FOR_MS);
    const stop = () => {
      drawn.disconnect();
      window.clearTimeout(given);
      forget();
    };
    drawn.observe(holder, { childList: true, subtree: true });
    return () => {
      drawn.disconnect();
      window.clearTimeout(given);
    };
  }, [key, page, t, setParameters]);
}
