/*
 * The drop that follows a finger along a rail of letters.
 *
 * It says which letter the finger is on, large enough to be read past the
 * thumb, and the letter rolls from one to the next like a counter. It moves
 * with the finger many times a second, so what changes with each movement is
 * written straight to the element, and only a change of letter is drawn again,
 * and then only this component, never the list the rail belongs to.
 */

import { forwardRef, useImperativeHandle, useRef, useState } from "react";

/** The size of the drop, which the stylesheet takes from the same number. */
const DROP_SIZE = 84;

export interface DropHandle {
  /** The finger is on the letter at `index`, at `y` down the screen. */
  move: (index: number, y: number) => void;
  /** The finger has gone. */
  release: () => void;
}

export const LetterDrop = forwardRef<DropHandle, { letters: string[] }>(
  function LetterDrop({ letters }, handle) {
    const box = useRef<HTMLDivElement>(null);
    const [index, setIndex] = useState(0);
    const [shown, setShown] = useState(false);

    useImperativeHandle(handle, () => ({
      move: (next, y) => {
        setIndex(next);
        setShown(true);
        box.current?.style.setProperty("--drop-y", `${y - DROP_SIZE / 2}px`);
      },
      release: () => setShown(false),
    }));

    return (
      <div className="letter-drop-place" ref={box} aria-hidden="true">
        <div className="letter-drop" data-shown={shown ? "yes" : "no"}>
          <div className="letter-drop-face">
            <div
              className="letter-drop-reel"
              style={{ ["--reel-at" as string]: index }}
            >
              {letters.map((letter) => (
                <span key={letter}>{letter}</span>
              ))}
            </div>
          </div>
        </div>
      </div>
    );
  },
);
