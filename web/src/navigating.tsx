/*
 * Going to another page from something that only goes there when pressed:
 * a card, the menu of a song, a name in a long list.
 *
 * The router's own way to navigate, and its links, are drawn again each time
 * the address changes, the player of music opening and closing included, so
 * a list of a thousand songs, each with its links and its menu, was drawn
 * again whole at every such step. What is handed out here is the same from
 * the first drawing to the last, and reaches the router only when pressed.
 */

import { createContext, useCallback, useContext, useLayoutEffect, useRef } from "react";
import type { AnchorHTMLAttributes, MouseEvent, ReactNode } from "react";
import { useNavigate } from "react-router-dom";
import type { NavigateOptions } from "react-router-dom";

type GoTo = (to: string, options?: NavigateOptions) => void;

const GoToContext = createContext<GoTo | null>(null);

/** Holds the router's way to navigate, and hands out one that never changes. */
export function GoToProvider({ children }: { children: ReactNode }) {
  const navigate = useNavigate();
  const latest = useRef(navigate);
  useLayoutEffect(() => {
    latest.current = navigate;
  });
  const goTo = useCallback<GoTo>((to, options) => void latest.current(to, options), []);
  return <GoToContext.Provider value={goTo}>{children}</GoToContext.Provider>;
}

export function useGoTo(): GoTo {
  const goTo = useContext(GoToContext);
  if (!goTo) {
    throw new Error("the way to navigate is not in place");
  }
  return goTo;
}

/** Whether a press on a link is one the browser should handle itself: another
 *  button, or a key held to open it elsewhere. */
function handledByTheBrowser(event: MouseEvent<HTMLAnchorElement>): boolean {
  return (
    event.defaultPrevented ||
    event.button !== 0 ||
    event.metaKey ||
    event.ctrlKey ||
    event.shiftKey ||
    event.altKey ||
    (event.currentTarget.target !== "" && event.currentTarget.target !== "_self")
  );
}

/** A link to a page of the app, by its full address, for the long lists. */
export function QuietLink({
  to,
  replace,
  onClick,
  ...rest
}: { to: string; replace?: boolean } & Omit<AnchorHTMLAttributes<HTMLAnchorElement>, "href">) {
  const goTo = useGoTo();
  return (
    <a
      {...rest}
      href={to}
      onClick={(event) => {
        onClick?.(event);
        if (!handledByTheBrowser(event)) {
          event.preventDefault();
          goTo(to, { replace });
        }
      }}
    />
  );
}
