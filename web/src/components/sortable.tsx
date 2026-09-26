/*
 * A list put in order by hand.
 *
 * A line is taken with the mouse anywhere on it, or by its grip on a touch
 * screen, where anywhere else has to stay free to scroll the page. The line
 * follows the hand and the others step aside as it passes them, so what the
 * list will be is seen before it is let go; let go, the line glides the last
 * few pixels into its place. From the keyboard, the grip moves its line with
 * the up and down arrows.
 *
 * The list says only what the new order is. Keeping it is the caller's,
 * which already knows where the order lives.
 */

import { useLayoutEffect, useRef, useState } from "react";
import type { KeyboardEvent, PointerEvent, ReactNode, TransitionEvent } from "react";
import { GripIcon } from "../icons";
import { useSettings } from "../settings";
import { landingPlace, leftToSettle, movedWithin, stepAside } from "../sorting";
import type { Line } from "../sorting";

/** How far a hand moves before a press becomes a drag. Any less, and a
 *  click on a line would nudge it. */
const A_DRAG = 4;

/** A press on a line, and what was measured of the list when it began. */
interface Press {
  pointer: number;
  from: number;
  startedAt: number;
  lines: Line[];
  moved: number;
  dragging: boolean;
}

/** The line being dragged, as the drawing needs it. */
interface Drag {
  from: number;
  to: number;
  moved: number;
  height: number;
}

/** The line just let go, still gliding into its place. Drawn first where the
 *  hand left it with nothing moving, then let glide: a list reordered while
 *  its lines still slide would slide every one of them a second time. */
interface Landing {
  key: string;
  offset: number;
  gliding: boolean;
}

export function Sortable<T>({
  items,
  keyOf,
  nameOf,
  onMove,
  lineClass,
  children,
}: {
  items: readonly T[];
  keyOf: (item: T) => string;
  /** What the line is called, for whoever moves it from the keyboard. */
  nameOf: (item: T) => string;
  onMove: (reordered: T[]) => void;
  lineClass?: (item: T) => string | undefined;
  /** What the line holds after its grip. */
  children: (item: T) => ReactNode;
}) {
  const { t } = useSettings();
  const list = useRef<HTMLOListElement>(null);
  const press = useRef<Press | null>(null);
  const [drag, setDrag] = useState<Drag | null>(null);
  const [landing, setLanding] = useState<Landing | null>(null);
  /* The line moved from the keyboard, whose grip is given the focus back:
     a line put somewhere else in the list can lose it on the way. */
  const refocus = useRef<string | null>(null);

  const fromTheTop = (clientY: number) => clientY - (list.current?.getBoundingClientRect().top ?? 0);

  const measured = (): Line[] => {
    const box = list.current;
    if (!box) {
      return [];
    }
    const top = box.getBoundingClientRect().top;
    return Array.from(box.children).map((child) => {
      const line = child.getBoundingClientRect();
      return { top: line.top - top, height: line.height };
    });
  };

  const take = (event: PointerEvent<HTMLLIElement>, from: number) => {
    if (event.button !== 0 || press.current) {
      return;
    }
    const target = event.target as HTMLElement;
    const onTheGrip = target.closest(".sortable-grip") !== null;
    // A control on the line keeps its own press, and a finger anywhere but
    // on the grip is scrolling the page.
    if (!onTheGrip && (event.pointerType !== "mouse" || target.closest("button, a, input, select, label"))) {
      return;
    }
    press.current = {
      pointer: event.pointerId,
      from,
      startedAt: fromTheTop(event.clientY),
      lines: measured(),
      moved: 0,
      dragging: false,
    };
    event.currentTarget.setPointerCapture(event.pointerId);
  };

  const follow = (event: PointerEvent<HTMLLIElement>) => {
    const now = press.current;
    if (!now || now.pointer !== event.pointerId) {
      return;
    }
    now.moved = fromTheTop(event.clientY) - now.startedAt;
    if (!now.dragging && Math.abs(now.moved) < A_DRAG) {
      return;
    }
    now.dragging = true;
    setLanding(null);
    setDrag({
      from: now.from,
      to: landingPlace(now.lines, now.from, now.moved),
      moved: now.moved,
      height: now.lines[now.from].height,
    });
  };

  /* Let go, or taken away by the browser: a drag called off lands back
     where it started, which is the same glide over a shorter way. */
  const letGo = (event: PointerEvent<HTMLLIElement>, dropped: boolean) => {
    const now = press.current;
    if (!now || now.pointer !== event.pointerId) {
      return;
    }
    press.current = null;
    if (!now.dragging) {
      return;
    }
    const to = dropped ? landingPlace(now.lines, now.from, now.moved) : now.from;
    setDrag(null);
    setLanding({
      key: keyOf(items[now.from]),
      offset: leftToSettle(now.lines, now.from, to, now.moved),
      gliding: false,
    });
    if (to !== now.from) {
      onMove(movedWithin(items, now.from, to));
    }
  };

  const settled = (event: TransitionEvent<HTMLLIElement>) => {
    if (event.target === event.currentTarget && event.propertyName === "transform") {
      setLanding(null);
    }
  };

  const nudge = (event: KeyboardEvent<HTMLButtonElement>, place: number) => {
    const step = event.key === "ArrowUp" ? -1 : event.key === "ArrowDown" ? 1 : 0;
    if (step === 0) {
      return;
    }
    event.preventDefault();
    const to = place + step;
    if (to < 0 || to >= items.length) {
      return;
    }
    refocus.current = keyOf(items[place]);
    onMove(movedWithin(items, place, to));
  };

  useLayoutEffect(() => {
    if (landing && !landing.gliding) {
      const frame = requestAnimationFrame(() =>
        setLanding((now) => now && { ...now, offset: 0, gliding: true }),
      );
      return () => cancelAnimationFrame(frame);
    }
  }, [landing]);

  useLayoutEffect(() => {
    const key = refocus.current;
    if (key === null) {
      return;
    }
    refocus.current = null;
    list.current
      ?.querySelector<HTMLElement>(`[data-sortable="${CSS.escape(key)}"] .sortable-grip`)
      ?.focus();
  }, [items]);

  return (
    <ol ref={list} className={`order sortable${drag ? " sortable-moving" : ""}`}>
      {items.map((item, place) => {
        const key = keyOf(item);
        const lands = landing?.key === key;
        const shift = drag
          ? stepAside(place, drag.from, drag.to, drag.moved, drag.height)
          : lands
            ? landing.offset
            : 0;
        const classes = [
          "order-line",
          "sortable-line",
          drag?.from === place ? "sortable-held" : undefined,
          lands && landing.gliding ? "sortable-landing" : undefined,
          lineClass?.(item),
        ]
          .filter(Boolean)
          .join(" ");
        const name = t("settings.move_line", { name: nameOf(item) });
        return (
          <li
            key={key}
            data-sortable={key}
            className={classes}
            style={shift === 0 ? undefined : { transform: `translateY(${shift}px)` }}
            onPointerDown={(event) => take(event, place)}
            onPointerMove={follow}
            onPointerUp={(event) => letGo(event, true)}
            onPointerCancel={(event) => letGo(event, false)}
            onTransitionEnd={lands ? settled : undefined}
          >
            <button
              type="button"
              className="sortable-grip"
              aria-label={name}
              title={name}
              onKeyDown={(event) => nudge(event, place)}
            >
              <GripIcon size={18} />
            </button>
            {children(item)}
          </li>
        );
      })}
    </ol>
  );
}
