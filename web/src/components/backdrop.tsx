/*
 * What is drawn behind a page, as the account chose it: one of the paintings
 * of light, or nothing over the plain surface of the theme.
 *
 * Fixed to the window rather than to the page, which styles/pages.css says:
 * each page that wears it puts it first, and it is the same painting on all
 * of them.
 */

import { useSettings } from "../settings";

export function PageBackdrop() {
  const { backdrop, backdropLight } = useSettings();
  if (backdrop === "none") {
    return null;
  }
  return <div className="home-backdrop drift" data-light={backdropLight} aria-hidden="true" />;
}
