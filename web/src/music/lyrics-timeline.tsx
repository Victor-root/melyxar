/*
 * The song in the lyrics editor laid out as an editing desk lays out a
 * timeline: a ruler of moments on top, a track of the lines, each a clip of
 * its words from where it starts to where the next one does, and the sound
 * of the song under them as one clip with its wave drawn in it.
 *
 * A clip's left edge is taken and dragged to move its line, or moved by the
 * arrows once it holds the focus; its right edge, which is where the next
 * line starts, to make the line last longer or less long. A press on a clip
 * chooses its line. A press anywhere else, the ruler and the sound
 * included, takes the song there and follows the hand while it is held, the
 * song heard at the pace of the hand (scrubbing.ts), the playhead running
 * across every track with its cap on the ruler.
 *
 * The playhead is moved by whatever plays the song, through `follow`, never
 * by drawing the timeline again: it is drawn again only when the lines, the
 * line chosen, its width or the stretch shown change, and it turns like a
 * page when the song runs off it.
 */

import { memo, useEffect, useImperativeHandle, useMemo, useRef, useState } from "react";
import type { KeyboardEvent, PointerEvent, Ref } from "react";
import type { Wording } from "../readable";
import type { SongWave } from "./api";
import type { EditedLine } from "./lyrics-editing";
import { stampOf } from "./lyrics-editing";
import type { HandSeen } from "./scrubbing";
import { WaveDrawing, WaveOverview, useWheelZoom } from "./wave-view";
import { asClock } from "../clock";
import { clockOf, paged, rulerOf, showing, zoomed } from "./wave-stretch";
import type { Stretch } from "./wave-stretch";

/** How much of the song the bar shows at first: enough for a few lines, few
 *  enough for each to be told from the next. */
const FIRST_SHOWN_SECONDS = 30;

/** How far an arrow moves a mark, and a shifted arrow, in milliseconds. */
const STEP_MS = 100;
const BIG_STEP_MS = 1000;

export interface TimelineHandle {
  /** Where the song has got to, in seconds, and whether it plays: a song
      playing turns the bar over when it runs off it. */
  follow: (seconds: number, playing: boolean) => void;
}

export interface TimelineActs {
  select: (key: number) => void;
  /** Moves a line to a moment, in milliseconds, as one step for as long as
      the same mark is held. */
  move: (key: number, at: number) => void;
  /** Makes a line end at a moment, in milliseconds, the same way. */
  end: (key: number, at: number) => void;
  /** The mark was let go. */
  settle: () => void;
  /** The playhead taken, dragged and let go: every place the hand was seen
      at since the last time, the latest last. */
  scrub: (phase: "take" | "drag" | "let go", seen: HandSeen[]) => void;
}

/** A stamped line where it falls, and where the next one does. */
interface Mark {
  line: EditedLine;
  number: number;
  at: number;
  until: number;
}

export const LyricsTimeline = memo(function LyricsTimeline({
  wave,
  length,
  lines,
  selected,
  acts,
  handle,
  t,
}: {
  wave: SongWave | null | undefined;
  /** In seconds. */
  length: number;
  lines: EditedLine[];
  selected: number | null;
  acts: TimelineActs;
  handle: Ref<TimelineHandle>;
  t: Wording;
}) {
  const [shown, setShown] = useState<Stretch>(() => ({ from: 0, to: Math.min(length, FIRST_SHOWN_SECONDS) }));
  const bar = useRef<HTMLDivElement>(null);
  const head = useRef<HTMLSpanElement>(null);
  const [width, setWidth] = useState(0);
  const shownNow = useRef(shown);
  shownNow.current = shown;
  const lastAt = useRef(0);
  /** The bar's place on the screen while the playhead is dragged along it. */
  const scrubbing = useRef<{ left: number; width: number } | null>(null);
  /** The end of a line held, and the bar's place on the screen, read when
      it is taken. */
  const held = useRef<{ key: number; end: "start" | "end"; left: number; width: number } | null>(null);
  useWheelZoom(bar, length, setShown);
  useEffect(() => {
    const element = bar.current;
    if (!element) {
      return;
    }
    const watcher = new ResizeObserver(([entry]) => setWidth(entry.contentRect.width));
    watcher.observe(element);
    return () => watcher.disconnect();
  }, []);

  useEffect(() => {
    setShown((was) => (was.to > length ? { from: 0, to: Math.min(length, FIRST_SHOWN_SECONDS) } : was));
  }, [length]);

  const place = (seconds: number) => {
    const element = head.current;
    if (element) {
      const { from, to } = shownNow.current;
      element.style.left = `${((seconds - from) / (to - from)) * 100}%`;
      element.style.visibility = seconds < from || seconds > to ? "hidden" : "";
    }
  };
  useImperativeHandle(
    handle,
    () => ({
      follow: (seconds, playing) => {
        lastAt.current = seconds;
        if (playing) {
          const next = paged(shownNow.current, length, seconds);
          if (next !== shownNow.current) {
            shownNow.current = next;
            setShown(next);
          }
        }
        place(seconds);
      },
    }),
    [length],
  );
  // The playhead stays where the song is when what is shown changes.
  useEffect(() => place(lastAt.current), [shown]);

  /* The line chosen is brought onto the bar, whichever way it was chosen. */
  const chosenAt = lines.find((line) => line.key === selected)?.at ?? null;
  useEffect(() => {
    if (chosenAt !== null && held.current === null) {
      setShown((was) => showing(was, length, chosenAt / 1000));
    }
  }, [chosenAt, length]);

  const marks = useMemo(() => {
    const stamped: Mark[] = [];
    lines.forEach((line, index) => {
      if (line.at !== null) {
        stamped.push({ line, number: index + 1, at: line.at, until: length * 1000 });
      }
    });
    stamped.sort((one, other) => one.at - other.at);
    stamped.forEach((mark, index) => {
      mark.until = stamped[index + 1]?.at ?? length * 1000;
    });
    return stamped;
  }, [lines, length]);

  const span = shown.to - shown.from;
  const across = (seconds: number) => ((seconds - shown.from) / span) * 100;
  const momentAt = (clientX: number, box: { left: number; width: number }) =>
    Math.min(Math.max(shown.from + ((clientX - box.left) / box.width) * span, 0), length);
  const seenAt = (event: { clientX: number; timeStamp: number }, box: { left: number; width: number }): HandSeen => ({
    seconds: momentAt(event.clientX, box),
    stamp: event.timeStamp,
  });

  const take = (key: number, end: "start" | "end", event: PointerEvent<HTMLElement>) => {
    const box = bar.current?.getBoundingClientRect();
    if (!box) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    event.currentTarget.setPointerCapture(event.pointerId);
    held.current = { key, end, left: box.left, width: box.width };
    acts.select(key);
  };
  const drag = (event: PointerEvent<HTMLElement>) => {
    const hand = held.current;
    if (hand && event.currentTarget.hasPointerCapture(event.pointerId)) {
      const at = momentAt(event.clientX, hand) * 1000;
      if (hand.end === "start") {
        acts.move(hand.key, at);
      } else {
        acts.end(hand.key, at);
      }
    }
  };
  const letGo = () => {
    held.current = null;
    acts.settle();
  };
  const keys = (mark: Mark, event: KeyboardEvent<HTMLElement>) => {
    const step = event.shiftKey ? BIG_STEP_MS : STEP_MS;
    const by = { ArrowLeft: -step, ArrowDown: -step, ArrowRight: step, ArrowUp: step }[event.key];
    if (by !== undefined) {
      event.preventDefault();
      event.stopPropagation();
      acts.move(mark.line.key, Math.min(Math.max(mark.at + by, 0), length * 1000));
    }
  };
  const zoom = (factor: number) =>
    setShown((was) => {
      const around = chosenAt !== null && chosenAt / 1000 >= was.from && chosenAt / 1000 <= was.to
        ? chosenAt / 1000
        : lastAt.current >= was.from && lastAt.current <= was.to
          ? lastAt.current
          : (was.from + was.to) / 2;
      return zoomed(was, length, factor, around);
    });

  const ofWhole = (ms: number) => `${(ms / 1000 / length) * 100}%`;
  const ruler = rulerOf(shown, width);
  const said = (seconds: number) => (ruler.step < 1 ? clockOf(seconds) : asClock(seconds));
  const soundFrom = Math.max(across(0), 0);
  const soundTo = Math.min(across(length), 100);

  return (
    <div className="lyrics-timeline">
      <div
        className="lyrics-desk"
        ref={bar}
        onPointerDown={(event) => {
          const box = event.currentTarget.getBoundingClientRect();
          event.currentTarget.setPointerCapture(event.pointerId);
          scrubbing.current = { left: box.left, width: box.width };
          acts.scrub("take", [seenAt(event, box)]);
        }}
        onPointerMove={(event) => {
          const hand = scrubbing.current;
          if (hand && event.currentTarget.hasPointerCapture(event.pointerId)) {
            // Every place the hand went through since the last frame, each
            // with its moment, rather than only the last.
            const gathered = event.nativeEvent.getCoalescedEvents?.() ?? [];
            acts.scrub("drag", (gathered.length > 0 ? gathered : [event]).map((one) => seenAt(one, hand)));
          }
        }}
        onPointerUp={(event) => {
          const hand = scrubbing.current;
          if (hand) {
            scrubbing.current = null;
            acts.scrub("let go", [seenAt(event, hand)]);
          }
        }}
        onPointerCancel={(event) => {
          const hand = scrubbing.current;
          if (hand) {
            scrubbing.current = null;
            acts.scrub("let go", [seenAt(event, hand)]);
          }
        }}
      >
        <div className="lyrics-ruler" aria-hidden="true">
          {ruler.ticks.map((at) => (
            <span key={`t${at}`} className="lyrics-ruler-tick" style={{ left: `${across(at)}%` }} />
          ))}
          {ruler.written.map((at) => (
            <span key={`w${at}`} className="lyrics-ruler-mark" style={{ left: `${across(at)}%` }}>
              <span className="lyrics-ruler-moment">{said(at)}</span>
            </span>
          ))}
        </div>

        <div className="lyrics-track lyrics-track-words">
          {marks.map((mark) => {
            const left = across(mark.at / 1000);
            const right = across(mark.until / 1000);
            if (right < 0 || left > 100) {
              return null;
            }
            const chosen = mark.line.key === selected;
            const from = Math.max(left, 0);
            const to = Math.min(right, 100);
            const words = mark.line.text || t("lyrics_editor.break");
            return (
              <span
                key={mark.line.key}
                className={`lyrics-clip${chosen ? " lyrics-clip-chosen" : ""}${mark.line.text ? "" : " lyrics-clip-break"}`}
                style={{ left: `${from}%`, width: `max(4px, calc(${to - from}cqw - 2px))` }}
                title={`${stampOf(mark.at)} · ${words}`}
                onPointerDown={(event) => {
                  event.stopPropagation();
                  acts.select(mark.line.key);
                }}
              >
                <span className="lyrics-clip-words">
                  <span className="lyrics-clip-number">{mark.number}</span>
                  {mark.line.text || "♪"}
                </span>
                {left >= 0 && (
                  <span
                    role="slider"
                    tabIndex={0}
                    className="lyrics-clip-edge lyrics-clip-start"
                    aria-label={t("lyrics_editor.line_number", { number: mark.number })}
                    aria-valuemin={0}
                    aria-valuemax={Math.round(length * 1000)}
                    aria-valuenow={mark.at}
                    aria-valuetext={stampOf(mark.at)}
                    onPointerDown={(event) => take(mark.line.key, "start", event)}
                    onPointerMove={drag}
                    onPointerUp={letGo}
                    onPointerCancel={letGo}
                    onKeyDown={(event) => keys(mark, event)}
                    onKeyUp={() => acts.settle()}
                    onFocus={() => acts.select(mark.line.key)}
                  />
                )}
                {right <= 100 && mark.until < length * 1000 && (
                  <span
                    className="lyrics-clip-edge lyrics-clip-end"
                    aria-hidden="true"
                    onPointerDown={(event) => take(mark.line.key, "end", event)}
                    onPointerMove={drag}
                    onPointerUp={letGo}
                    onPointerCancel={letGo}
                  />
                )}
              </span>
            );
          })}
        </div>

        <div className="lyrics-track lyrics-track-sound">
          <span className="lyrics-sound" style={{ left: `${soundFrom}%`, right: `${100 - soundTo}%` }}>
            <WaveDrawing wave={wave} shown={shown} swing className="lyrics-sound-wave" />
          </span>
        </div>

        <span className="wave-head lyrics-head" ref={head} />
      </div>
      <WaveOverview wave={wave} length={length} shown={shown} setShown={setShown} zoom={zoom} swing t={t}>
        {marks.map((mark) => (
          <span
            key={mark.line.key}
            className={`lyrics-strip-mark${mark.line.key === selected ? " lyrics-strip-mark-chosen" : ""}`}
            style={{ left: ofWhole(mark.at) }}
          />
        ))}
      </WaveOverview>
    </div>
  );
});
