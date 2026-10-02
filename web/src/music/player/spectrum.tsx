/*
 * A soft spectrum behind the bar of the player: thin bars rising from its
 * foot in the accent, blurred and faded towards the top, only there to dress
 * the bar. While it is a trial the levels are made up, not read from the
 * song: `nextTargets` is the one place that says where they come from.
 *
 * It costs as little as drawing can. New targets come a few times a second,
 * the bars glide towards them between two of those, the frame rate is held at
 * thirty, and nothing runs while the song is paused, once the bars have
 * settled, or while the tab is hidden.
 */

import { useEffect, useRef } from "react";

/** How many bands the levels are given in, stretched over the whole width. */
const BANDS = 32;
/** The width one bar is drawn at, in pixels, and the gap left after it,
 *  whatever the width of the bar. */
const BAR_WIDTH = 8;
const BAR_GAP = 2;
const NEW_TARGETS_EVERY_MS = 140;
const FRAME_EVERY_MS = 33;
/** How much of the way to its target a bar goes in a frame. */
const GLIDE = 0.22;
/** Below this, a bar is taken for settled. */
const SETTLED = 0.004;

/** Where the bars are heading: strongest in the low bands, as music is. */
function nextTargets(): number[] {
  return Array.from({ length: BANDS }, (_, band) => {
    const tilt = 1 - (band / BANDS) * 0.6;
    return (0.4 + Math.random() * 0.6) * tilt;
  });
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

/** The level at a place along the width, between the two bands around it. */
export function levelAt(levels: number[], place: number): number {
  const at = place * (levels.length - 1);
  const low = Math.floor(at);
  const high = Math.min(levels.length - 1, low + 1);
  return levels[low] + (levels[high] - levels[low]) * (at - low);
}

export function Spectrum({ playing }: { playing: boolean }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  /* Kept across a pause, so the bars glide down rather than drop. */
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
      const bars = Math.max(1, Math.floor(width / (BAR_WIDTH + BAR_GAP)));
      for (let bar = 0; bar < bars; bar++) {
        const level = levelAt(levels, bars === 1 ? 0 : bar / (bars - 1));
        const tall = level * height;
        context.fillRect(bar * (BAR_WIDTH + BAR_GAP), height - tall, BAR_WIDTH, tall);
      }
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
