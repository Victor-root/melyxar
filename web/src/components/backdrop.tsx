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

export function PageBackdrop({ inPlace = false }: { inPlace?: boolean }) {
  const { backdrop, backdropLight } = useSettings();
  if (backdrop === "none") {
    return null;
  }
  const drawn =
    backdrop === "library" ? (
      <div className="home-backdrop" aria-hidden="true">
        <DrawnLibrary />
      </div>
    ) : (
      <div className="home-backdrop drift" data-light={backdropLight} aria-hidden="true" />
    );
  /* Put on the body rather than in the page: it is the window's, and what
     stands over the page, such as the fade of the page into the player of
     music, must never take it along. A page that covers the whole window
     draws its own, in place, under its own words. */
  return inPlace ? drawn : createPortal(drawn, document.body);
}
