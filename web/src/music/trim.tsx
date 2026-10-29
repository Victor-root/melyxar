/*
 * One song cut short at either end, from the tag manager or straight from
 * the song's menu: the silence a ripper left before the first note, most of
 * the time.
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

import { useEffect, useMemo, useRef, useState } from "react";
import type { KeyboardEvent, PointerEvent, ReactNode } from "react";
import { useAccount } from "../account";
import { refusalOf } from "../asking";
import { Modal } from "../components/modal";
import { Toggle } from "../components/panel";
import { refusalKey } from "../i18n";
import { PlayIcon, ScissorsIcon } from "../icons";
import { useToast } from "../notifications/toasts";
import { useRunning } from "../running";
import { useSettings } from "../settings";
import { music as server } from "./api";
import type { Song } from "./api";
import { HeardSong } from "./heard-song";
import { PauseIcon } from "./player/icons";
import { useMusic } from "./player/player";
import type { MenuLine } from "./song-menu";
import { SHORTEST_CUT_SECONDS, keptWithin, soundBetween } from "./trim-cut";
import { WaveDrawing, WaveOverview, useSongWave, useWheelZoom } from "./wave-view";
import { clockOf, showing, zoomed } from "./wave-stretch";
import type { Stretch } from "./wave-stretch";

/** How far a key moves a handle, and a shifted key. */
const STEP = 0.1;
const BIG_STEP = 1;

/** How long each end is listened to. */
const HEARD_SECONDS = 6;

type End = "from" | "to";

/** The line of the menu of one song that cuts it, and the window it opens,
 *  for an account that may write the tags of songs. The screens showing the
 *  song read it again once the server has read its library again. */
export function useSongTrim(songs: Song[]): { lines: MenuLine[]; dialog: ReactNode } {
  const { t } = useSettings();
  const { account } = useAccount();
  const [trimming, setTrimming] = useState(false);
  const song = songs.length === 1 ? songs[0] : null;
  const length = song?.duration_ms != null ? song.duration_ms / 1000 : song?.seconds;
  if (!song || length == null || account?.may_edit_tags !== true) {
    return { lines: [], dialog: null };
  }
  return {
    lines: [{ key: "trim", said: t("music.trim.open"), mark: <ScissorsIcon size={17} />, act: () => setTrimming(true) }],
    dialog: trimming && <TrimSong song={song.id} title={song.title} length={length} onClose={() => setTrimming(false)} />,
  };
}

export function TrimSong({
  song,
  title,
  length,
  keepACopy: keptByTheTags,
  onTrimmed,
  onClose,
}: {
  song: string;
  title: string;
  /** How long the song plays now, in seconds. */
  length: number;
  /** Chosen by the tag manager it is opened from; asked here otherwise. */
  keepACopy?: boolean;
  /** Told how long the song plays once it is cut. */
  onTrimmed?: (length: number) => void;
  onClose: () => void;
}) {
  const { t } = useSettings();
  const [keptHere, setKeptHere] = useState(true);
  const keepACopy = keptByTheTags ?? keptHere;
  const toast = useToast();
  const { watch } = useRunning();
  const { playing, toggle } = useMusic();
  const [cut, setCut] = useState({ from: 0, to: length });
  const whole = useMemo(() => ({ from: 0, to: length }), [length]);
  const [shown, setShown] = useState<Stretch>(whole);
  const wave = useSongWave(song);
  const [heard, setHeard] = useState<End | null>(null);
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);
  const bar = useRef<HTMLDivElement>(null);
  const head = useRef<HTMLSpanElement>(null);
  const hearing = useMemo(() => new HeardSong(song), [song]);
  const frame = useRef(0);
  /** The bar's place on the screen, read when a handle is taken. */
  const held = useRef<{ end: End; left: number; width: number } | null>(null);
  /** The handle moved last, which the zoom buttons zoom around. */
  const focus = useRef<End>("from");
  /** What the bar shows, for the playhead, which follows outside renders. */
  const showingNow = useRef(shown);
  showingNow.current = shown;

  useWheelZoom(bar, length, setShown);

  const stopListening = () => {
    cancelAnimationFrame(frame.current);
    hearing.stop();
    setHeard(null);
  };
  useEffect(() => () => {
    cancelAnimationFrame(frame.current);
    hearing.stop();
  }, [hearing]);

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
    hearing.play(start);
    setHeard(end);
    const follow = () => {
      const at = hearing.position();
      if (head.current) {
        const { from, to } = showingNow.current;
        head.current.style.left = `${((at - from) / (to - from)) * 100}%`;
        head.current.style.visibility = at < from || at > to ? "hidden" : "";
      }
      if (at >= until || !hearing.playing) {
        stopListening();
        return;
      }
      frame.current = requestAnimationFrame(follow);
    };
    frame.current = requestAnimationFrame(follow);
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
      onTrimmed?.(cut.to - cut.from);
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

      <div className="wave-bar trim-bar" ref={bar} onPointerDown={pressBar}>
        <WaveDrawing wave={wave} shown={shown} className="wave-bar-drawing" />
        <span className="trim-out trim-out-start" style={{ width: across(cut.from) }} />
        <span className="trim-out trim-out-end" style={{ left: across(cut.to) }} />
        <span className="trim-kept" style={{ left: across(cut.from), right: `calc(100% - ${across(cut.to)})` }} />
        {close && (
          <>
            <span className="wave-edge wave-edge-from">{clockOf(shown.from)}</span>
            <span className="wave-edge wave-edge-to">{clockOf(shown.to)}</span>
          </>
        )}
        <span className="wave-head" ref={head} hidden={heard === null} />
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

      <WaveOverview wave={wave} length={length} shown={shown} setShown={setShown} zoom={zoom} t={t}>
        <span className="trim-strip-out trim-strip-out-start" style={{ width: ofWhole(cut.from) }} />
        <span className="trim-strip-out trim-strip-out-end" style={{ left: ofWhole(cut.to) }} />
      </WaveOverview>

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

      {keptByTheTags === undefined && (
        <label className="trim-option">
          <Toggle label={t("music.tag.keep_a_copy")} checked={keptHere} onChange={setKeptHere} />
          <span title={t("music.tag.keep_a_copy_why")}>{t("music.tag.keep_a_copy")}</span>
        </label>
      )}

      {wave === null && <p className="trim-note">{t("music.trim.no_wave")}</p>}
      {refused && <p className="notice">{t(refusalKey(refused))}</p>}
    </Modal>
  );
}
