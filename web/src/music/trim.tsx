/*
 * One song cut short at either end, from the tag manager: the silence a
 * ripper left before the first note, most of the time.
 *
 * The song's loudness, read from its file a hundred times a second, is drawn
 * along a bar, so where it is silent is seen at a glance, and a handle at
 * each end of the bar says where the song is cut. The bar zooms in down to a
 * second and a half, a strip of the whole song under it showing and moving
 * the stretch shown. Each cut can be listened to before anything is written,
 * and the silence at both ends found in one press. The file itself is cut by
 * the server.
 *
 * The wave is drawn again only when the bar changes size or what it shows;
 * moving a handle moves the handle and the two veils over what is cut, never
 * the drawing.
 */

import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { KeyboardEvent, PointerEvent } from "react";
import { refusalOf } from "../asking";
import { Modal } from "../components/modal";
import { refusalKey } from "../i18n";
import { PlayIcon, ScissorsIcon, ZoomInIcon, ZoomOutIcon } from "../icons";
import { useToast } from "../notifications/toasts";
import { useRunning } from "../running";
import { useSettings } from "../settings";
import { music as server } from "./api";
import type { SongWave } from "./api";
import { PauseIcon } from "./player/icons";
import { soundOf, useMusic } from "./player/player";
import {
  CLOSEST_SECONDS,
  SHORTEST_CUT_SECONDS,
  barsOf,
  clockOf,
  keptWithin,
  panned,
  showing,
  soundBetween,
  zoomed,
} from "./trim-cut";
import type { Stretch } from "./trim-cut";

/** How far a key moves a handle, and a shifted key. */
const STEP = 0.1;
const BIG_STEP = 1;

/** How long each end is listened to. */
const HEARD_SECONDS = 6;

/** How much closer a press of a zoom button brings the bar. */
const ZOOM_STEP = 2;

/** How much closer a pixel of the wheel brings it. */
const ZOOM_A_PIXEL = 0.0025;

/** The width of one bar of the wave and the gap after it, in pixels. */
const BAR = 3;
const GAP = 1.5;

type End = "from" | "to";

const NO_WAVE: SongWave = { momentsASecond: 1, levels: new Uint8Array(0) };

export function TrimSong({
  song,
  title,
  length,
  keepACopy,
  onTrimmed,
  onClose,
}: {
  song: string;
  title: string;
  /** How long the song plays now, in seconds. */
  length: number;
  keepACopy: boolean;
  /** Told how long the song plays once it is cut. */
  onTrimmed: (length: number) => void;
  onClose: () => void;
}) {
  const { t } = useSettings();
  const toast = useToast();
  const { watch } = useRunning();
  const { playing, toggle } = useMusic();
  const [cut, setCut] = useState({ from: 0, to: length });
  const whole = useMemo(() => ({ from: 0, to: length }), [length]);
  const [shown, setShown] = useState<Stretch>(whole);
  // Nothing yet while it is read, and null for a file that could not be.
  const [wave, setWave] = useState<SongWave | null | undefined>(undefined);
  const [heard, setHeard] = useState<End | null>(null);
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);
  const bar = useRef<HTMLDivElement>(null);
  const head = useRef<HTMLSpanElement>(null);
  const listening = useRef<{ audio: HTMLAudioElement; frame: number } | null>(null);
  /** The bar's place on the screen, read when a handle is taken. */
  const held = useRef<{ end: End; left: number; width: number } | null>(null);
  /** The handle moved last, which the zoom buttons zoom around. */
  const focus = useRef<End>("from");
  /** What the bar shows, for the playhead, which follows outside renders. */
  const showingNow = useRef(shown);
  showingNow.current = shown;

  useEffect(() => {
    const stop = new AbortController();
    server.songWave(song, stop.signal).then(setWave, () => {
      if (!stop.signal.aborted) {
        setWave(null);
      }
    });
    return () => stop.abort();
  }, [song]);

  // The wheel zooms around the moment under the pointer, and sideways it
  // moves along, a trackpad's pinch included. Listened to by hand, as the
  // page must not scroll under it.
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
  }, [length]);

  const stopListening = () => {
    const now = listening.current;
    if (now) {
      cancelAnimationFrame(now.frame);
      now.audio.pause();
      now.audio.removeAttribute("src");
      now.audio.load();
    }
    listening.current = null;
    setHeard(null);
  };
  useEffect(() => stopListening, []);

  const listen = (end: End) => {
    if (heard === end) {
      stopListening();
      return;
    }
    stopListening();
    if (playing) {
      toggle();
    }
    const start = end === "from" ? cut.from : Math.max(cut.from, cut.to - HEARD_SECONDS);
    const until = end === "from" ? Math.min(cut.to, cut.from + HEARD_SECONDS) : cut.to;
    const audio = new Audio(soundOf(song, start));
    const now = { audio, frame: 0 };
    listening.current = now;
    setHeard(end);
    // A song the browser plays as it lies is moved in; one converted on the
    // way already begins where it was asked to, and counts from there.
    let offset = 0;
    audio.addEventListener("loadedmetadata", () => {
      if (Number.isFinite(audio.duration)) {
        audio.currentTime = start;
      } else {
        offset = start;
      }
      void audio.play().catch(stopListening);
    });
    const follow = () => {
      const at = offset + audio.currentTime;
      if (head.current) {
        const { from, to } = showingNow.current;
        head.current.style.left = `${((at - from) / (to - from)) * 100}%`;
        head.current.style.visibility = at < from || at > to ? "hidden" : "";
      }
      if (at >= until || audio.ended) {
        stopListening();
        return;
      }
      now.frame = requestAnimationFrame(follow);
    };
    now.frame = requestAnimationFrame(follow);
  };

  const moveTo = (end: End, at: number) => {
    focus.current = end;
    setCut((was) =>
      end === "from"
        ? keptWithin(at, was.to, length)
        : { from: was.from, to: Math.min(Math.max(at, was.from + SHORTEST_CUT_SECONDS), length) },
    );
  };
  const span = shown.to - shown.from;
  /** A moment of the song, from where it lies across the bar. */
  const momentAt = (clientX: number, box: { left: number; width: number }) =>
    shown.from + ((clientX - box.left) / box.width) * span;

  const take = (end: End, event: PointerEvent<HTMLElement>) => {
    const box = bar.current?.getBoundingClientRect();
    if (!box) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    event.currentTarget.setPointerCapture(event.pointerId);
    held.current = { end, left: box.left, width: box.width };
  };
  const drag = (event: PointerEvent<HTMLElement>) => {
    const hand = held.current;
    if (hand && event.currentTarget.hasPointerCapture(event.pointerId)) {
      moveTo(hand.end, momentAt(event.clientX, hand));
    }
  };
  const letGo = () => {
    held.current = null;
  };
  // A press on the bar itself brings the nearer handle there.
  const pressBar = (event: PointerEvent<HTMLDivElement>) => {
    const at = momentAt(event.clientX, event.currentTarget.getBoundingClientRect());
    moveTo(Math.abs(at - cut.from) <= Math.abs(at - cut.to) ? "from" : "to", at);
  };
  const keys = (end: End, event: KeyboardEvent<HTMLElement>) => {
    const step = event.shiftKey ? BIG_STEP : STEP;
    const by = { ArrowLeft: -step, ArrowDown: -step, ArrowRight: step, ArrowUp: step }[event.key];
    if (by === undefined) {
      return;
    }
    event.preventDefault();
    const at = Math.min(Math.max(cut[end] + by, 0), length);
    moveTo(end, at);
    setShown((was) => showing(was, length, at));
  };

  const zoom = (factor: number) =>
    setShown((was) => {
      const around = cut[focus.current];
      const on = around >= was.from && around <= was.to;
      return zoomed(was, length, factor, on ? around : (was.from + was.to) / 2);
    });

  /** Where the stretch shown was taken on the strip of the whole song. */
  const grabbed = useRef<number | null>(null);
  const takeStrip = (event: PointerEvent<HTMLDivElement>) => {
    event.currentTarget.setPointerCapture(event.pointerId);
    const at = momentOnStrip(event);
    const on = at >= shown.from && at <= shown.to;
    const grab = on ? at - shown.from : span / 2;
    grabbed.current = grab;
    if (!on) {
      setShown((was) => panned(was, length, at - grab - was.from));
    }
  };
  const dragStrip = (event: PointerEvent<HTMLDivElement>) => {
    const grab = grabbed.current;
    if (grab !== null && event.currentTarget.hasPointerCapture(event.pointerId)) {
      const at = momentOnStrip(event);
      setShown((was) => panned(was, length, at - grab - was.from));
    }
  };
  const letGoOfStrip = () => {
    grabbed.current = null;
  };
  const momentOnStrip = (event: PointerEvent<HTMLDivElement>) => {
    const box = event.currentTarget.getBoundingClientRect();
    return ((event.clientX - box.left) / box.width) * length;
  };

  const silence = wave ? soundBetween(wave, length) : null;
  const unchanged = cut.from < 0.05 && cut.to > length - 0.05;

  const save = async () => {
    stopListening();
    setBusy(true);
    setRefused(null);
    try {
      await server.trimSong(song, Math.round(cut.from * 1000), Math.round(cut.to * 1000), keepACopy);
      toast({ state: "ok", title: t("music.trim.done", { title }) });
      // The server reads the library again once the file is cut.
      watch();
      onTrimmed(cut.to - cut.from);
      onClose();
    } catch (error) {
      setRefused(refusalOf(error));
    } finally {
      setBusy(false);
    }
  };

  /** Where a moment lies across the bar, kept on it. */
  const across = (seconds: number) => `${Math.min(Math.max(((seconds - shown.from) / span) * 100, 0), 100)}%`;
  const off = (seconds: number) => seconds < shown.from - 0.001 || seconds > shown.to + 0.001;
  const ofWhole = (seconds: number) => `${(seconds / length) * 100}%`;
  const close = span < length - 0.001;

  return (
    <Modal
      title={t("music.trim.title")}
      onClose={onClose}
      className="trim"
      footer={
        <>
          <span className={`trim-warning${keepACopy ? "" : " trim-warning-for-good"}`}>
            {t(keepACopy ? "music.trim.kept_copy" : "music.trim.for_good")}
          </span>
          <button
            type="button"
            className="button button-accent"
            disabled={busy || unchanged}
            onClick={() => void save()}
          >
            <ScissorsIcon size={16} />
            {t("music.trim.save")}
          </button>
        </>
      }
    >
      <div className="trim-song">
        <span className="trim-song-title">{title}</span>
        <span className="trim-song-length">{clockOf(length)}</span>
      </div>

      <div className="trim-bar" ref={bar} onPointerDown={pressBar}>
        <WaveDrawing wave={wave} shown={shown} className="trim-wave" />
        <span className="trim-out trim-out-start" style={{ width: across(cut.from) }} />
        <span className="trim-out trim-out-end" style={{ left: across(cut.to) }} />
        <span className="trim-kept" style={{ left: across(cut.from), right: `calc(100% - ${across(cut.to)})` }} />
        {close && (
          <>
            <span className="trim-edge trim-edge-from">{clockOf(shown.from)}</span>
            <span className="trim-edge trim-edge-to">{clockOf(shown.to)}</span>
          </>
        )}
        <span className="trim-head" ref={head} hidden={heard === null} />
        {(["from", "to"] as const).map((end) => (
          <span
            key={end}
            role="slider"
            tabIndex={0}
            className={`trim-handle trim-handle-${end}`}
            style={{ left: across(cut[end]) }}
            hidden={off(cut[end])}
            aria-label={t(end === "from" ? "music.trim.handle_start" : "music.trim.handle_end")}
            aria-valuemin={0}
            aria-valuemax={Math.round(length * 10) / 10}
            aria-valuenow={Math.round(cut[end] * 10) / 10}
            aria-valuetext={clockOf(cut[end])}
            onPointerDown={(event) => take(end, event)}
            onPointerMove={drag}
            onPointerUp={letGo}
            onPointerCancel={letGo}
            onKeyDown={(event) => keys(end, event)}
          />
        ))}
      </div>

      <div className="trim-zoom">
        <button
          type="button"
          className="trim-zoom-button"
          disabled={!close}
          title={t("music.trim.zoom_out")}
          aria-label={t("music.trim.zoom_out")}
          onClick={() => zoom(1 / ZOOM_STEP)}
        >
          <ZoomOutIcon size={18} />
        </button>
        <div
          className="trim-strip"
          onPointerDown={takeStrip}
          onPointerMove={dragStrip}
          onPointerUp={letGoOfStrip}
          onPointerCancel={letGoOfStrip}
        >
          <WaveDrawing wave={wave} shown={whole} className="trim-strip-wave" />
          <span className="trim-strip-out trim-strip-out-start" style={{ width: ofWhole(cut.from) }} />
          <span className="trim-strip-out trim-strip-out-end" style={{ left: ofWhole(cut.to) }} />
          <span
            className="trim-window"
            style={{ left: ofWhole(shown.from), right: `calc(100% - ${ofWhole(shown.to)})` }}
          />
        </div>
        <button
          type="button"
          className="trim-zoom-button"
          disabled={span <= Math.min(CLOSEST_SECONDS, length) + 0.001}
          title={t("music.trim.zoom_in")}
          aria-label={t("music.trim.zoom_in")}
          onClick={() => zoom(ZOOM_STEP)}
        >
          <ZoomInIcon size={18} />
        </button>
      </div>

      <div className="trim-clocks">
        <span className="trim-clock">
          <span className="trim-clock-name">{t("music.trim.start")}</span>
          <span className="trim-clock-time">{clockOf(cut.from)}</span>
        </span>
        <span className="trim-clock trim-clock-kept">
          <span className="trim-clock-name">{t("music.trim.length")}</span>
          <span className="trim-clock-time">{clockOf(cut.to - cut.from)}</span>
        </span>
        <span className="trim-clock">
          <span className="trim-clock-name">{t("music.trim.end")}</span>
          <span className="trim-clock-time">{clockOf(cut.to)}</span>
        </span>
      </div>

      <div className="trim-tools">
        <button type="button" className="button button-small" onClick={() => listen("from")}>
          {heard === "from" ? <PauseIcon size={14} /> : <PlayIcon size={14} />}
          {t("music.trim.hear_start")}
        </button>
        <button
          type="button"
          className="button button-small"
          disabled={!silence}
          onClick={() => silence && setCut(silence)}
        >
          <ScissorsIcon size={14} />
          {t("music.trim.silence")}
        </button>
        <button type="button" className="button button-small" onClick={() => listen("to")}>
          {heard === "to" ? <PauseIcon size={14} /> : <PlayIcon size={14} />}
          {t("music.trim.hear_end")}
        </button>
      </div>

      {wave === null && <p className="trim-note">{t("music.trim.no_wave")}</p>}
      {refused && <p className="notice">{t(refusalKey(refused))}</p>}
    </Modal>
  );
}

/** The wave drawn for a stretch of the song, again only when the stretch or
 *  the size of the drawing changes. */
function WaveDrawing({
  wave,
  shown,
  className,
}: {
  /** Nothing yet while it is read, and null for a file that could not be. */
  wave: SongWave | null | undefined;
  shown: Stretch;
  className: string;
}) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const size = useRef<{ width: number; height: number } | null>(null);
  const drawn = useRef({ wave, shown });
  drawn.current = { wave, shown };

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
    const count = Math.max(1, Math.floor(width / (BAR + GAP)));
    const bars = barsOf(drawn.current.wave ?? NO_WAVE, drawn.current.shown, count);
    const step = width / count;
    const thick = Math.min(BAR, step * 0.7);
    bars.forEach((level, index) => {
      const tall = Math.max(2, level * height * 0.92);
      context.beginPath();
      context.roundRect(index * step, (height - tall) / 2, thick, tall, thick / 2);
      context.fill();
    });
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

  useLayoutEffect(paint, [wave, shown]);

  return <canvas className={`${className}${wave === undefined ? " trim-wave-waiting" : ""}`} ref={canvas} />;
}
