/*
 * What the browser is told so it can install the interface as an application,
 * on a computer as on a telephone.
 *
 * The manifest and the icons are drawn by the server in the colour of the
 * logo: an installed application keeps the address of its icon, never a
 * picture made by a tab. The colours are the theme's own, read off the page,
 * and travel in the address, because the browser fetches all of it without
 * the session's cookie.
 */

/** Where the manifest and the icons are served. */
const THE_APP = "/api/v1/public/app";

/** A colour of the theme as six hexadecimal digits, or nothing when the
 *  stylesheet writes it some other way. */
function tokenOf(name: string): string | null {
  const written = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return /^#[0-9a-f]{6}$/i.test(written) ? written.slice(1).toLowerCase() : null;
}

/** The one element of the head that says this, made the first time. */
function headElement<T extends HTMLElement>(selector: string, made: () => T): T {
  const found = document.head.querySelector<T>(selector);
  if (found) {
    return found;
  }
  const element = made();
  document.head.append(element);
  return element;
}

function aLink(rel: string): () => HTMLLinkElement {
  return () => {
    const link = document.createElement("link");
    link.rel = rel;
    return link;
  };
}

/**
 * Points the browser at the manifest and the touch icon in the logo's colour
 * as it stands now. Called whenever the accent changes, after the logo's
 * colour is written on the page.
 */
export function markTheApp(): void {
  const mark = tokenOf("--mark-colour");
  const ground = tokenOf("--app-ground");
  if (!mark || !ground) {
    return;
  }
  const manifest = `${THE_APP}/manifest?mark=${mark}&ground=${ground}`;
  const touch = `${THE_APP}/icon/${mark}/${ground}`;
  const manifestLink = headElement('link[rel="manifest"]', aLink("manifest"));
  if (manifestLink.getAttribute("href") !== manifest) {
    manifestLink.href = manifest;
  }
  // The icon a telephone puts on its home screen when the page is added to
  // it by hand, which some read rather than the manifest.
  const touchLink = headElement('link[rel="apple-touch-icon"]', aLink("apple-touch-icon"));
  if (touchLink.getAttribute("href") !== touch) {
    touchLink.href = touch;
  }
}

/**
 * Colours the browser's own bar, above the page on a telephone and around
 * the window of the installed application, with the ground of the page as
 * the theme draws it now. Called whenever the theme changes.
 */
export function colourTheWindow(): void {
  const ground = tokenOf("--surface-deep");
  if (!ground) {
    return;
  }
  const meta = headElement('meta[name="theme-color"]', () => {
    const made = document.createElement("meta");
    made.name = "theme-color";
    return made;
  });
  meta.content = `#${ground}`;
}
