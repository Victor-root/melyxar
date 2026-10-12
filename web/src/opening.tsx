/*
 * Where the app opens: the home page, or on a phone the library chosen to
 * open on in its place.
 *
 * Only as the app opens, once, and without a step back to the home page
 * behind it. Coming back home afterwards is not opening the app: the Home
 * button would otherwise lead nowhere else.
 */

import { useEffect } from "react";
import { Navigate, useLocation } from "react-router-dom";
import type { Library } from "./api";
import { ON_A_PHONE, useMediaQuery } from "./media-query";
import { HomePage } from "./pages/home";
import { useSettings } from "./settings";

/** Whether this visit began on the home page, where the installed app opens,
 *  and has not left it yet. */
let stillOpening = window.location.pathname === "/";

/** Forgets the opening as soon as anything else is shown. Held above every
 *  page, since the page that opened may be gone before the server answers. */
export function useOpeningPassesOnLeaving() {
  const { pathname } = useLocation();
  useEffect(() => {
    if (pathname !== "/") {
      stillOpening = false;
    }
  }, [pathname]);
}

/**
 * The home page, unless the app is opening on a phone given a library to
 * open on.
 *
 * The choice this device kept answers at once. A device that never kept one
 * waits for the account's answer with the home page shown, and still moves if
 * the answer names a library. With a library to go to, nothing is drawn
 * until the libraries are known, so the home page does not flash first; one
 * no longer there leaves the home page.
 */
export function Opening({ libraries }: { libraries: Library[] }) {
  const { startLibraryOnPhone, adopted } = useSettings();
  const onAPhone = useMediaQuery(ON_A_PHONE);
  const opening = stillOpening && onAPhone;
  const wanted = opening ? startLibraryOnPhone : null;
  const known = libraries.length > 0;

  useEffect(() => {
    if (opening && adopted && (wanted === null || known)) {
      stillOpening = false;
    }
  }, [opening, adopted, wanted, known]);

  if (wanted !== null && !known) {
    return <main className="page" aria-busy="true" />;
  }
  if (wanted !== null && libraries.some((library) => library.id === wanted)) {
    return <Navigate to={`/library/${wanted}`} replace />;
  }
  return <HomePage libraries={libraries} />;
}
