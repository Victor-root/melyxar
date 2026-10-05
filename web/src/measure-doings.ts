/*
 * What the person did, and what the page set moving, noted for the recorder.
 *
 * A stalled frame says when, never why. Next to what was going on at that
 * moment (the pointer entering a card, a click, a transition starting) it
 * does. Nothing typed or read is kept: an element is named by its tag and
 * first class, never by its text, which can be a title.
 */

import type { Doing } from "./measure-report";

/** The same doing again this soon is the same doing: a transition starts on
 *  every card of a row at once. */
const REPEAT_MS = 100;

/** How many doings are kept: past this, the oldest half goes. */
const KEPT = 4000;

/** What can be pointed at, closest to the element hit. */
const POINTED_AT = ".card, button, a, input, select, textarea, [role]";

/** The nearest thing that can be pointed at around what an event hit. */
function pointedAt(target: EventTarget | null): Element | null {
  return target instanceof Element ? (target.closest(POINTED_AT) ?? target) : null;
}

/** An element in a few words, or the nearest thing that can be pointed at
 *  around it. */
export function describe(target: EventTarget | null, nearest = true): string {
  const element = nearest ? pointedAt(target) : target;
  if (!(element instanceof Element)) {
    return "?";
  }
  const kind = element.classList[0];
  return kind ? `${element.localName}.${kind}` : element.localName;
}

/** Notes into `doings` what goes on from now on, in order of time, and gives
 *  back the way to note one more thing there. */
export function noteDoings(doings: Doing[]): (at: number, what: string) => void {
  const note = (at: number, what: string) => {
    const last = doings[doings.length - 1];
    if (last && last.what === what && at - last.at < REPEAT_MS) {
      return;
    }
    if (doings.length >= KEPT) {
      doings.splice(0, KEPT / 2);
    }
    doings.push({ at, what });
  };
  const listen = (type: string, what: (event: Event) => string) =>
    document.addEventListener(
      type,
      (event) => {
        const said = what(event);
        if (said) {
          note(event.timeStamp, said);
        }
      },
      { capture: true, passive: true },
    );

  /* The pointer crossing the parts of one card is one hover. */
  let hovered: Element | null = null;
  listen("pointerover", (event) => {
    const over = pointedAt(event.target);
    if (over === hovered) {
      return "";
    }
    hovered = over;
    return `over ${describe(over, false)}`;
  });
  listen("pointerdown", (event) => `press ${describe(event.target)}`);
  listen("click", (event) => `click ${describe(event.target)}`);
  listen("keydown", (event) => `key in ${describe(event.target)}`);
  listen(
    "transitionrun",
    (event) =>
      `transition ${(event as TransitionEvent).propertyName} on ${describe(event.target, false)}`,
  );
  listen(
    "animationstart",
    (event) =>
      `animation ${(event as AnimationEvent).animationName} on ${describe(event.target, false)}`,
  );

  return note;
}
