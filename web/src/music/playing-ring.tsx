/*
 * The mark of the song playing: a wave standing round its red button, as an
 * equaliser would stand round it, the smooth curve through the tips of
 * spikes that each rise and fall on their own, the two halves mirroring one
 * another. It follows nothing of the song: it goes through shapes drawn
 * once at random, the browser easing from one to the next. Only the one
 * line that is playing has it, a small picture redrawn as it eases; paused,
 * it holds still and draws back behind the button.
 */

import { useEffect, useRef } from "react";
import { mirroredHeights, radialWave } from "./ring-wave";

/** How many spikes the wave stands on, and how far in it starts, as a share
 *  of how far it reaches: at the button's edge. */
const SPIKES = 20;
const INNER = 0.62;

/** The shapes it goes through, and how long it takes from one to the next. */
const SHAPES = mirroredHeights(SPIKES, 8).map((heights) => radialWave(heights, INNER));
const STEP_MS = 380;

export function PlayingRing({ playing }: { playing: boolean }) {
  const wave = useRef<SVGPathElement>(null);
  const motion = useRef<Animation | null>(null);

  useEffect(() => {
    const path = wave.current;
    if (!path || window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      return;
    }
    const frames = [...SHAPES, SHAPES[0]].map((shape) => ({ d: `path("${shape}")`, easing: "ease-in-out" }));
    motion.current = path.animate(frames, { duration: STEP_MS * SHAPES.length, iterations: Infinity });
    return () => {
      motion.current?.cancel();
      motion.current = null;
    };
  }, []);

  useEffect(() => {
    if (playing) {
      motion.current?.play();
    } else {
      motion.current?.pause();
    }
  }, [playing]);

  return (
    <span className={`playing-ring${playing ? " playing-ring-on" : ""}`} aria-hidden="true">
      <svg viewBox="-1 -1 2 2">
        <path ref={wave} d={SHAPES[0]} />
      </svg>
    </span>
  );
}
