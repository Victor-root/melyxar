/*
 * The ring the keyboard draws round what has the focus, kept for the keyboard.
 *
 * Leaving the window by its own keys, Alt and Tab on a computer, has the
 * browser count the way back as the keyboard's: what was last pressed with
 * the mouse got its ring on the return, every time, for a viewer who never
 * used the keyboard to get there. Whatever a hand put the focus on is let go
 * as the window is left, so nothing is ringed on the return. Something the
 * keyboard reached keeps the focus, and its ring; so does a field being typed
 * in, which keeps its caret.
 */

/** Whether the focus was last moved by a hand rather than by the keyboard. */
let byHand = false;

function typedIn(element: Element): boolean {
  return (
    element instanceof HTMLTextAreaElement ||
    element instanceof HTMLSelectElement ||
    (element instanceof HTMLInputElement && element.type !== "range") ||
    (element instanceof HTMLElement && element.isContentEditable)
  );
}

export function keepRingsForTheKeyboard(): void {
  document.addEventListener("pointerdown", () => (byHand = true), true);
  document.addEventListener(
    "keydown",
    (event) => {
      if (event.key === "Tab" && !event.altKey && !event.metaKey && !event.ctrlKey) {
        byHand = false;
      }
    },
    true,
  );
  window.addEventListener("blur", () => {
    const held = document.activeElement;
    if (byHand && held instanceof HTMLElement && held !== document.body && !typedIn(held)) {
      held.blur();
    }
  });
}
