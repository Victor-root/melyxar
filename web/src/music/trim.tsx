/*
 * One song cut short at either end, from the tag manager: the silence a
 * ripper left before the first note, most of the time.
 *
 * The song's loudness is drawn along a bar, so where it is silent is seen at
 * a glance, and a handle at each end of the bar says where the song is cut.
 * Each cut can be listened to before anything is written, and the silence at
 * both ends found in one press. The file itself is cut by the server.
 *
 * The wave is drawn once for the size of the bar; moving a handle moves the
 * handle and the two veils over what is cut, never the drawing.
 */

import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { KeyboardEvent, PointerEvent } from "react";
import { refusalOf } from "../asking";
import { Modal } from "../components/modal";
import { refusalKey } from "../i18n";
import { PlayIcon, ScissorsIcon } from "../icons";
import { useToast } from "../notifications/toasts";
import { useRunning } from "../running";
import { useSettings } from "../settings";
import { music as server } from "./api";
import { PauseIcon } from "./player/icons";
import { soundOf, useMusic } from "./player/player";
import { SHORTEST_CUT_SECONDS, clockOf, keptWithin, loudnessOf, soundBetween } from "./trim-cut";

/** How far a key moves a handle, and a shifted key. */
const STEP = 0.1;
const BIG_STEP = 1;

/** How long each end is listened to. */
const HEARD_SECONDS = 6;

/** The width of one bar of the wave and the gap after it, in pixels. */
const BAR = 3;
const GAP = 1.5;

type End = "from" | "to";

interface Loudness {
  levels: number[];
  aSecond: number;
}

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
  // Nothing yet while it is asked for, and null for a song never analysed.
  const [loudness, setLoudness] = useState<Loudness | null | undefined>(undefined);
  const [heard, setHeard] = useState<End | null>(null);
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);
  const bar = useRef<HTMLDivElement>(null);
  const wave = useRef<HTMLCanvasElement>(null);
  const head = useRef<HTMLSpanElement>(null);
  const listening = useRef<{ audio: HTMLAudioElement; frame: number } | null>(null);
  /** The bar's place on the screen, read when a handle is taken. */
  const held = useRef<{ end: End; left: number; width: number } | null>(null);

  useEffect(() => {
    const stop = new AbortController();
    server.spectrum(song, stop.signal).then(
      (spectrum) =>
        setLoudness(spectrum ? { levels: loudnessOf(spectrum), aSecond: spectrum.framesASecond } : null),
      () => {
        if (!stop.signal.aborted) {
          setLoudness(null);
        }
      },
    );
    return () => stop.abort();
  }, [song]);

  useLayoutEffect(() => {
    const canvas = wave.current;
    const context = canvas?.getContext("2d");
    if (!canvas || !context) {
      return;
    }
    const draw = (width: number, height: number) => {
      const ratio = window.devicePixelRatio || 1;
      canvas.width = Math.round(width * ratio);
      canvas.height = Math.round(height * ratio);
      context.setTransform(ratio, 0, 0, ratio, 0, 0);
      context.clearRect(0, 0, width, height);
      context.fillStyle = getComputedStyle(canvas).color;
      const bars = Math.max(1, Math.floor(width / (BAR + GAP)));
      const levels = loudness?.levels ?? [];
      for (let index = 0; index < bars; index += 1) {
        const first = Math.floor((index / bars) * levels.length);
        const last = Math.max(first + 1, Math.floor(((index + 1) / bars) * levels.length));
        let level = 0;
        for (let moment = first; moment < last && moment < levels.length; moment += 1) {
          level = Math.max(level, levels[moment]);
        }
        const tall = Math.max(2, level * height * 0.86);
        context.beginPath();
        context.roundRect(index * (BAR + GAP), (height - tall) / 2, BAR, tall, BAR / 2);
        context.fill();
      }
    };
    const watcher = new ResizeObserver(([entry]) => draw(entry.contentRect.width, entry.contentRect.height));
    watcher.observe(canvas);
    return () => watcher.disconnect();
  }, [loudness]);

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
        head.current.style.left = `${(at / length) * 100}%`;
      }
      if (at >= until || audio.ended) {
        stopListening();
        return;
      }
      now.frame = requestAnimationFrame(follow);
    };
    now.frame = requestAnimationFrame(follow);
  };

  const moveTo = (end: End, at: number) =>
    setCut((was) =>
      end === "from"
        ? keptWithin(at, was.to, length)
        : { from: was.from, to: Math.min(Math.max(at, was.from + SHORTEST_CUT_SECONDS), length) },
    );

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
      moveTo(hand.end, ((event.clientX - hand.left) / hand.width) * length);
    }
  };
  const letGo = () => {
    held.current = null;
  };
  // A press on the bar itself brings the nearer handle there.
  const pressBar = (event: PointerEvent<HTMLDivElement>) => {
    const box = event.currentTarget.getBoundingClientRect();
    const at = ((event.clientX - box.left) / box.width) * length;
    moveTo(Math.abs(at - cut.from) <= Math.abs(at - cut.to) ? "from" : "to", at);
  };
  const keys = (end: End, event: KeyboardEvent<HTMLElement>) => {
    const step = event.shiftKey ? BIG_STEP : STEP;
    const by = { ArrowLeft: -step, ArrowDown: -step, ArrowRight: step, ArrowUp: step }[event.key];
    if (by === undefined) {
      return;
    }
    event.preventDefault();
    moveTo(end, cut[end] + by);
  };

  const silence = loudness ? soundBetween(loudness.levels, loudness.aSecond, length) : null;
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

  const share = (seconds: number) => `${(seconds / length) * 100}%`;

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
        <canvas className={`trim-wave${loudness === undefined ? " trim-wave-waiting" : ""}`} ref={wave} />
        <span className="trim-out trim-out-start" style={{ width: share(cut.from) }} />
        <span className="trim-out trim-out-end" style={{ width: share(length - cut.to) }} />
        <span className="trim-kept" style={{ left: share(cut.from), right: share(length - cut.to) }} />
        <span className="trim-head" ref={head} hidden={heard === null} />
        {(["from", "to"] as const).map((end) => (
          <span
            key={end}
            role="slider"
            tabIndex={0}
            className={`trim-handle trim-handle-${end}`}
            style={{ left: share(cut[end]) }}
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

      {loudness === null && <p className="trim-note">{t("music.trim.no_wave")}</p>}
      {refused && <p className="notice">{t(refusalKey(refused))}</p>}
    </Modal>
  );
}
