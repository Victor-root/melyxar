/*
 * Putting text on the clipboard, by whichever way this browser allows.
 *
 * Melyxar is reached at an address on the local network, which a browser does
 * not call a secure page, and the modern clipboard interface is refused on
 * one. So the older way is tried next, and it is the way that actually works
 * here. Anything that skips it ends up showing the text and asking somebody to
 * copy it by hand, which is not copying.
 */

/** Tries every way a browser offers, newest first. */
export async function putOnTheClipboard(text: string): Promise<boolean> {
  try {
    if (window.isSecureContext && navigator.clipboard) {
      await navigator.clipboard.writeText(text);
      return true;
    }
  } catch {
    // Refused, which is one of the two ordinary answers here.
  }

  // The way that predates the clipboard being an interface of its own, and
  // the only one a page served over a plain address still has.
  try {
    const field = document.createElement("textarea");
    field.value = text;
    field.setAttribute("readonly", "");
    field.style.position = "fixed";
    field.style.opacity = "0";
    document.body.appendChild(field);
    field.select();
    const taken = document.execCommand("copy");
    document.body.removeChild(field);
    return taken;
  } catch {
    return false;
  }
}
