/*
 * The tooltips of the interface, drawn by the page rather than by the
 * browser: a native one is laid by the system over whatever it likes and was
 * seen cut by the line above it and by a cover. This one stands on top of
 * everything, outside every box that could clip it, and is kept inside the
 * window.
 *
 * Nothing asks for it: every element with a `title` has its tooltip drawn
 * here. The title is held aside while the pointer is over the element, so the
 * browser draws nothing of its own, and put back when it leaves.
 */

import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

/** How long the pointer rests on something before it is explained. */
const DELAY_MS = 450;
/** How far from the edge of the window a tooltip is kept. */
const EDGE = 8;
const GAP = 8;

interface Shown {
  text: string;
  anchor: DOMRect;
}

export function Tooltips() {
  const [shown, setShown] = useState<Shown | null>(null);
  const [at, setAt] = useState<{ left: number; top: number } | null>(null);
  const bubble = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let element: HTMLElement | null = null;
    let timer: ReturnType<typeof setTimeout> | undefined;

    const leave = () => {
      clearTimeout(timer);
      if (element?.dataset.tip !== undefined) {
        element.setAttribute("title", element.dataset.tip);
        delete element.dataset.tip;
      }
      element = null;
      setShown(null);
      setAt(null);
    };

    const over = (event: PointerEvent) => {
      if (event.pointerType !== "mouse") {
        return;
      }
      const target = (event.target as Element | null)?.closest?.<HTMLElement>("[title]") ?? null;
      if (target === element) {
        return;
      }
      leave();
      const text = target?.getAttribute("title");
      if (!target || !text) {
        return;
      }
      element = target;
      target.dataset.tip = text;
      target.removeAttribute("title");
      timer = setTimeout(() => setShown({ text, anchor: target.getBoundingClientRect() }), DELAY_MS);
    };

    document.addEventListener("pointerover", over);
    document.addEventListener("pointerdown", leave, true);
    document.addEventListener("keydown", leave, true);
    window.addEventListener("scroll", leave, true);
    window.addEventListener("blur", leave);
    return () => {
      document.removeEventListener("pointerover", over);
      document.removeEventListener("pointerdown", leave, true);
      document.removeEventListener("keydown", leave, true);
      window.removeEventListener("scroll", leave, true);
      window.removeEventListener("blur", leave);
      leave();
    };
  }, []);

  /* Placed once it is measured: above the element when there is room, below
     it otherwise, and always inside the window. */
  useLayoutEffect(() => {
    const box = bubble.current;
    if (!shown || !box) {
      return;
    }
    const { width, height } = box.getBoundingClientRect();
    const { anchor } = shown;
    const above = anchor.top - GAP - height >= EDGE;
    const top = above ? anchor.top - GAP - height : anchor.bottom + GAP;
    const left = Math.min(
      Math.max(EDGE, anchor.left + anchor.width / 2 - width / 2),
      window.innerWidth - width - EDGE,
    );
    setAt({ left, top });
  }, [shown]);

  if (!shown) {
    return null;
  }
  return createPortal(
    <div
      ref={bubble}
      className="tooltip"
      role="tooltip"
      style={{ left: at?.left ?? 0, top: at?.top ?? 0, visibility: at ? "visible" : "hidden" }}
    >
      {shown.text}
    </div>,
    document.body,
  );
}
