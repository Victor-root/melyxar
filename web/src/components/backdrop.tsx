/*
 * What is drawn behind a page, as the account chose it: one of the paintings
 * of light, the drawn shelf of the door, or nothing over the plain surface of
 * the theme.
 *
 * Fixed to the window rather than to the page, which styles/pages.css says:
 * each page that wears it puts it first, and it is the same painting on all
 * of them.
 */

import { createPortal } from "react-dom";
import { useSettings } from "../settings";
import { DrawnLibrary } from "./door-background";

export function PageBackdrop() {
  const { backdrop, backdropLight } = useSettings();
  if (backdrop === "none") {
    return null;
  }
  /* Put on the body rather than in the page: it is the window's, and what
     stands over the page, such as the fade of the page into the player of
     music, must never take it along. */
  return createPortal(
    backdrop === "library" ? (
      <div className="home-backdrop" aria-hidden="true">
        <DrawnLibrary />
      </div>
    ) : (
      <div className="home-backdrop drift" data-light={backdropLight} aria-hidden="true" />
    ),
    document.body,
  );
}
