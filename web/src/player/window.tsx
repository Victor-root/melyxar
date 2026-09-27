/*
 * A window standing on the picture: moved by its title bar, sized by its
 * edges, shut only by its own cross.
 *
 * What is opened this way is watched while the film plays and while other
 * panels come and go, so nothing but its cross closes it, and it never
 * leaves the picture it stands on.
 */

import { useRef, useState } from "react";
import type { ReactNode } from "react";

import { framed } from "./frame";
import type { Frame, Hold } from "./frame";

/** The edges and corners a hand can pull the window by. */
const EDGES: Exclude<Hold, "move">[] = ["n", "s", "e", "w", "ne", "nw", "se", "sw"];

/**
 * A window moved by its title bar and sized by its edges, never past the
 * picture it stands on.
 *
 * It opens where the stylesheet puts it. Its place and size are read once,
 * as a hand first takes hold, and held from then on; the rest of the way is
 * arithmetic on the pointer, and nothing is measured while it moves.
 */
function useFramedByHand() {
  const sheet = useRef<HTMLElement>(null);
  const [frame, setFrame] = useState<Frame | null>(null);
  const holding = useRef<{
    hold: Hold;
    x: number;
    y: number;
    from: Frame;
    room: { width: number; height: number };
  } | null>(null);

  const grip = (hold: Hold) => ({
    onPointerDown: (event: React.PointerEvent<HTMLElement>) => {
      const it = sheet.current;
      const room = it?.parentElement;
      // The cross in the bar is a button, not a place to take hold of.
      if (event.button !== 0 || (event.target as Element).closest("button") || !it || !room) {
        return;
      }
      const at = it.getBoundingClientRect();
      const within = room.getBoundingClientRect();
      holding.current = {
        hold,
        x: event.clientX,
        y: event.clientY,
        from: {
          left: at.left - within.left,
          top: at.top - within.top,
          width: at.width,
          height: at.height,
        },
        room: { width: within.width, height: within.height },
      };
      event.currentTarget.setPointerCapture(event.pointerId);
      event.preventDefault();
    },
    onPointerMove: (event: React.PointerEvent<HTMLElement>) => {
      const held = holding.current;
      if (held) {
        setFrame(framed(held.hold, held.from, held.room, event.clientX - held.x, event.clientY - held.y));
      }
    },
    onPointerUp: () => {
      holding.current = null;
    },
    onPointerCancel: () => {
      holding.current = null;
    },
  });

  const style: React.CSSProperties | undefined = frame
    ? {
        left: frame.left,
        top: frame.top,
        width: frame.width,
        height: frame.height,
        right: "auto",
        bottom: "auto",
        maxHeight: "none",
      }
    : undefined;

  return { sheet, style, grip };
}

export function PlayerWindow({
  title,
  closeLabel,
  className,
  onClose,
  children,
}: {
  title: string;
  closeLabel: string;
  /** Where it first opens, when not where every window does. */
  className?: string;
  onClose: () => void;
  children: ReactNode;
}) {
  const framing = useFramedByHand();
  return (
    <aside
      className={className ? `player-window ${className}` : "player-window"}
      aria-label={title}
      ref={framing.sheet}
      style={framing.style}
    >
      {EDGES.map((edge) => (
        <span
          key={edge}
          className={`player-window-edge player-window-edge-${edge}`}
          aria-hidden="true"
          {...framing.grip(edge)}
        />
      ))}
      <div className="player-window-head" {...framing.grip("move")}>
        <strong>{title}</strong>
        <button className="player-button" onClick={onClose} aria-label={closeLabel}>
          ✕
        </button>
      </div>
      {children}
    </aside>
  );
}
