/*
 * A song's wave along a bar, as the tools that place something on a song
 * draw it: cutting it short, and placing the lines of its lyrics.
 *
 * The bar zooms in down to a second and a half, by the wheel, a trackpad's
 * pinch or two buttons either side of a strip of the whole song, which shows
 * and moves the stretch the bar shows. The wave is drawn again only when the
 * bar changes size or what it shows.
 */

import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { Dispatch, PointerEvent, ReactNode, RefObject, SetStateAction } from "react";
import { ZoomInIcon, ZoomOutIcon } from "../icons";
import type { Wording } from "../readable";
import { music as server } from "./api";
import type { SongWave } from "./api";
import { CLOSEST_SECONDS, barsOf, panned, swingOf, zoomed } from "./wave-stretch";
import type { Stretch } from "./wave-stretch";

/** How much closer a press of a zoom button brings the bar. */
const ZOOM_STEP = 2;

/** How much closer a pixel of the wheel brings it. */
const ZOOM_A_PIXEL = 0.0025;


const NO_WAVE: SongWave = { momentsASecond: 1, levels: new Uint8Array(0) };

/** The wave of a song, read once for as long as it is shown: nothing yet
 *  while it is read, and null for a file that could not be. */
export function useSongWave(song: string): SongWave | null | undefined {
  const [wave, setWave] = useState<SongWave | null | undefined>(undefined);
  useEffect(() => {
    const stop = new AbortController();
    setWave(undefined);
    server.songWave(song, stop.signal).then(setWave, () => {
      if (!stop.signal.aborted) {
        setWave(null);
      }
    });
    return () => stop.abort();
  }, [song]);
  return wave;
}

/** The wheel over the bar zooms around the moment under the pointer, and
 *  sideways it moves along. Listened to by hand, as the page must not scroll
 *  under it. */
export function useWheelZoom(
  bar: RefObject<HTMLElement | null>,
  length: number,
  setShown: Dispatch<SetStateAction<Stretch>>,
) {
  useEffect(() => {
    const element = bar.current;
    if (!element) {
      return;
    }
    const wheel = (event: WheelEvent) => {
      event.preventDefault();
      const box = element.getBoundingClientRect();
      const lines = event.deltaMode === WheelEvent.DOM_DELTA_LINE ? 16 : 1;
      const share = (event.clientX - box.left) / box.width;
      if (event.shiftKey || Math.abs(event.deltaX) > Math.abs(event.deltaY)) {
        const by = (event.shiftKey ? event.deltaY : event.deltaX) * lines;
        setShown((was) => panned(was, length, (by / box.width) * (was.to - was.from)));
        return;
      }
      const factor = Math.exp(-event.deltaY * lines * ZOOM_A_PIXEL);
      setShown((was) => zoomed(was, length, factor, was.from + share * (was.to - was.from)));
    };
    element.addEventListener("wheel", wheel, { passive: false });
    return () => element.removeEventListener("wheel", wheel);
  }, [bar, length, setShown]);
}

/** The whole song in a strip between the two zoom buttons, the stretch the
 *  bar shows framed in it and moved by dragging. */
export function WaveOverview({
  wave,
  length,
  shown,
  setShown,
  zoom,
  swing = false,
  t,
  children,
}: {
  wave: SongWave | null | undefined;
  /** Drawn as the sound swings rather than in decibels. */
  swing?: boolean;
  length: number;
  shown: Stretch;
  setShown: Dispatch<SetStateAction<Stretch>>;
  /** Zooms by a factor, over one being closer, around what matters most. */
  zoom: (factor: number) => void;
  t: Wording;
  /** Drawn over the strip, under the frame. */
  children?: ReactNode;
}) {
  const span = shown.to - shown.from;
  const whole = useRef<Stretch>({ from: 0, to: length });
  if (whole.current.to !== length) {
    whole.current = { from: 0, to: length };
  }
  /** Where the stretch shown was taken on the strip. */
  const grabbed = useRef<number | null>(null);
  const momentOn = (event: PointerEvent<HTMLDivElement>) => {
    const box = event.currentTarget.getBoundingClientRect();
    return ((event.clientX - box.left) / box.width) * length;
  };
  const take = (event: PointerEvent<HTMLDivElement>) => {
    event.currentTarget.setPointerCapture(event.pointerId);
    const at = momentOn(event);
    const on = at >= shown.from && at <= shown.to;
    const grab = on ? at - shown.from : span / 2;
    grabbed.current = grab;
    if (!on) {
      setShown((was) => panned(was, length, at - grab - was.from));
    }
  };
  const drag = (event: PointerEvent<HTMLDivElement>) => {
    const grab = grabbed.current;
    if (grab !== null && event.currentTarget.hasPointerCapture(event.pointerId)) {
      const at = momentOn(event);
      setShown((was) => panned(was, length, at - grab - was.from));
    }
  };
  const letGo = () => {
    grabbed.current = null;
  };
  const ofWhole = (seconds: number) => `${(seconds / length) * 100}%`;

  return (
    <div className="wave-zoom">
      <button
        type="button"
        className="wave-zoom-button"
        disabled={span >= length - 0.001}
        title={t("music.wave.zoom_out")}
        aria-label={t("music.wave.zoom_out")}
        onClick={() => zoom(1 / ZOOM_STEP)}
      >
        <ZoomOutIcon size={18} />
      </button>
      <div className="wave-strip" onPointerDown={take} onPointerMove={drag} onPointerUp={letGo} onPointerCancel={letGo}>
        <WaveDrawing wave={wave} shown={whole.current} swing={swing} className="wave-strip-drawing" />
        {children}
        <span className="wave-window" style={{ left: ofWhole(shown.from), right: `calc(100% - ${ofWhole(shown.to)})` }} />
      </div>
      <button
        type="button"
        className="wave-zoom-button"
        disabled={span <= Math.min(CLOSEST_SECONDS, length) + 0.001}
        title={t("music.wave.zoom_in")}
        aria-label={t("music.wave.zoom_in")}
        onClick={() => zoom(ZOOM_STEP)}
      >
        <ZoomInIcon size={18} />
      </button>
    </div>
  );
}

/** The wave drawn for a stretch of the song, one solid shape mirrored either
 *  side of its middle as an editing desk draws sound, the loudest moment
 *  under each pixel deciding how far it reaches. Drawn again only when the
 *  stretch or the size of the drawing changes. */
export function WaveDrawing({
  wave,
  shown,
  swing = false,
  className,
}: {
  /** Nothing yet while it is read, and null for a file that could not be. */
  wave: SongWave | null | undefined;
  shown: Stretch;
  /** Drawn as the sound swings, each attack standing out, rather than in
      decibels, where a fade and a silence do. */
  swing?: boolean;
  className: string;
}) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const size = useRef<{ width: number; height: number } | null>(null);
  // The loudest moment of the song, which swings all the way.
  const loudest = useMemo(() => (wave ? wave.levels.reduce((most, level) => Math.max(most, level), 0) / 255 : 1), [wave]);
  const drawn = useRef({ wave, shown, swing, loudest });
  drawn.current = { wave, shown, swing, loudest };

  const paint = () => {
    const element = canvas.current;
    const context = element?.getContext("2d");
    if (!element || !context || !size.current) {
      return;
    }
    const { width, height } = size.current;
    const ratio = window.devicePixelRatio || 1;
    element.width = Math.round(width * ratio);
    element.height = Math.round(height * ratio);
    context.setTransform(ratio, 0, 0, ratio, 0, 0);
    context.clearRect(0, 0, width, height);
    context.fillStyle = getComputedStyle(element).color;
    const count = Math.max(1, Math.round(width));
    const levels = barsOf(drawn.current.wave ?? NO_WAVE, drawn.current.shown, count);
    const step = width / count;
    const middle = height / 2;
    const reach = levels.map((level) => Math.max(0.5, (drawn.current.swing ? swingOf(level, drawn.current.loudest) : level) * middle * 0.96));
    context.beginPath();
    context.moveTo(0, middle);
    reach.forEach((up, index) => context.lineTo(index * step + step / 2, middle - up));
    context.lineTo(width, middle);
    for (let index = reach.length - 1; index >= 0; index -= 1) {
      context.lineTo(index * step + step / 2, middle + reach[index]);
    }
    context.closePath();
    context.fill();
  };

  useLayoutEffect(() => {
    const element = canvas.current;
    if (!element) {
      return;
    }
    const watcher = new ResizeObserver(([entry]) => {
      size.current = { width: entry.contentRect.width, height: entry.contentRect.height };
      paint();
    });
    watcher.observe(element);
    return () => watcher.disconnect();
  }, []);

  useLayoutEffect(paint, [wave, shown, swing]);

  return <canvas className={`wave-drawing ${className}${wave === undefined ? " wave-drawing-waiting" : ""}`} ref={canvas} />;
}
