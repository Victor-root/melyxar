/*
 * A soft spectrum behind the bar of the player: a smooth wave rising from its
 * foot in the accent, blurred and faded towards the top, only there to dress
 * the bar. While it is a trial the levels are made up, not read from the
 * song: `nextTargets` is the one place that says where they come from.
 *
 * It costs as little as drawing can. New targets come a few times a second,
 * the wave glides towards them between two of those, the frame rate is held at
 * thirty, and nothing runs while the song is paused, once the wave has
 * settled, or while the tab is hidden.
 */

import { useEffect, useRef } from "react";

/** How many bands the levels are given in, the wave running through them
 *  over the whole width. */
const BANDS = 32;
const NEW_TARGETS_EVERY_MS = 140;
const FRAME_EVERY_MS = 33;
/** How much of the way to its target a bar goes in a frame. */
const GLIDE = 0.22;
/** Below this, a bar is taken for settled. */
const SETTLED = 0.004;

/** Where the wave is heading: strongest in the low bands, as music is, and
 *  with each band keeping close to its neighbours. */
function nextTargets(): number[] {
  const raw = Array.from({ length: BANDS }, (_, band) => {
    const tilt = 1 - (band / BANDS) * 0.6;
    return (0.4 + Math.random() * 0.6) * tilt;
  });
  return raw.map((level, band) => (raw[Math.max(0, band - 1)] + level * 2 + raw[Math.min(BANDS - 1, band + 1)]) / 4);
}

/** One step of every level towards its target, and whether all have settled
 *  at the target. */
export function approach(levels: number[], targets: number[]): boolean {
  let settled = true;
  for (let band = 0; band < levels.length; band++) {
    levels[band] += (targets[band] - levels[band]) * GLIDE;
    if (Math.abs(targets[band] - levels[band]) > SETTLED) {
      settled = false;
    }
  }
  return settled;
}

export function Spectrum({ playing }: { playing: boolean }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  /* Kept across a pause, so the wave glides down rather than drop. */
  const kept = useRef(new Array<number>(BANDS).fill(0));

  useEffect(() => {
    const element = canvas.current;
    const context = element?.getContext("2d");
    if (!element || !context) {
      return;
    }
    const levels = kept.current;
    const color = getComputedStyle(element).color;
    let targets = new Array<number>(BANDS).fill(0);
    let width = 0;
    let height = 0;
    let frame = 0;
    let lastDrawn = 0;
    let lastTargets = 0;

    const draw = () => {
      context.clearRect(0, 0, width, height);
      context.fillStyle = color;
      const step = width / (BANDS - 1);
      const top = (band: number) => height - levels[band] * height;
      context.beginPath();
      context.moveTo(0, height);
      context.lineTo(0, top(0));
      for (let band = 0; band < BANDS - 1; band++) {
        context.quadraticCurveTo(
          band * step,
          top(band),
          (band + 0.5) * step,
          (top(band) + top(band + 1)) / 2,
        );
      }
      context.lineTo(width, top(BANDS - 1));
      context.lineTo(width, height);
      context.closePath();
      context.fill();
    };

    const tick = (now: number) => {
      frame = 0;
      if (document.hidden) {
        return;
      }
      if (now - lastDrawn >= FRAME_EVERY_MS) {
        lastDrawn = now;
        if (playing && now - lastTargets >= NEW_TARGETS_EVERY_MS) {
          lastTargets = now;
          targets = nextTargets();
        }
        const settled = approach(levels, targets);
        draw();
        if (!playing && settled) {
          return;
        }
      }
      frame = requestAnimationFrame(tick);
    };

    const start = () => {
      if (!frame && !document.hidden) {
        frame = requestAnimationFrame(tick);
      }
    };

    if (!playing) {
      targets = new Array<number>(BANDS).fill(0);
    }
    const watcher = new ResizeObserver(([entry]) => {
      width = Math.round(entry.contentRect.width);
      height = Math.round(entry.contentRect.height);
      element.width = width;
      element.height = height;
      draw();
    });
    watcher.observe(element);
    document.addEventListener("visibilitychange", start);
    start();
    return () => {
      watcher.disconnect();
      document.removeEventListener("visibilitychange", start);
      cancelAnimationFrame(frame);
    };
  }, [playing]);

  return <canvas ref={canvas} className="music-bar-spectrum" aria-hidden="true" />;
}
