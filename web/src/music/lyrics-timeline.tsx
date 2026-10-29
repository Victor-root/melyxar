/*
 * The song's wave in the lyrics editor, each stamped line a mark on it where
 * it starts, its words running up to the next. A mark is taken and dragged
 * to move its line, or moved by the arrows once it holds the focus; a press
 * on the wave anywhere else takes the song there.
 *
 * The playhead is moved by whatever plays the song, through `follow`, never
 * by drawing the bar again: the bar is drawn again only when the lines, the
 * line chosen or the stretch shown change, and it turns like a page when the
 * song runs off it.
 */

import { memo, useEffect, useImperativeHandle, useMemo, useRef, useState } from "react";
import type { KeyboardEvent, PointerEvent, Ref } from "react";
import type { Wording } from "../readable";
import type { SongWave } from "./api";
import type { EditedLine } from "./lyrics-editing";
import { stampOf } from "./lyrics-editing";
import { WaveDrawing, WaveOverview, useWheelZoom } from "./wave-view";
import { clockOf, paged, showing, zoomed } from "./wave-stretch";
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
  /** The mark was let go. */
  settle: () => void;
  seek: (seconds: number) => void;
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
  const shownNow = useRef(shown);
  shownNow.current = shown;
  const lastAt = useRef(0);
  /** The bar's place on the screen, read when a mark is taken. */
  const held = useRef<{ key: number; left: number; width: number } | null>(null);
  useWheelZoom(bar, length, setShown);

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

  const take = (key: number, event: PointerEvent<HTMLElement>) => {
    const box = bar.current?.getBoundingClientRect();
    if (!box) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    event.currentTarget.setPointerCapture(event.pointerId);
    held.current = { key, left: box.left, width: box.width };
    acts.select(key);
  };
  const drag = (event: PointerEvent<HTMLElement>) => {
    const hand = held.current;
    if (hand && event.currentTarget.hasPointerCapture(event.pointerId)) {
      acts.move(hand.key, momentAt(event.clientX, hand) * 1000);
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
  const close = span < length - 0.001;

  return (
    <div className="lyrics-timeline">
      <div
        className="wave-bar lyrics-timeline-bar"
        ref={bar}
        onPointerDown={(event) => acts.seek(momentAt(event.clientX, event.currentTarget.getBoundingClientRect()))}
      >
        <WaveDrawing wave={wave} shown={shown} className="wave-bar-drawing lyrics-timeline-wave" />
        {marks.map((mark) => {
          const left = across(mark.at / 1000);
          if (left < -0.001 || left > 100.001) {
            return null;
          }
          const room = across(Math.min(mark.until / 1000, shown.to)) - left;
          const chosen = mark.line.key === selected;
          return (
            <span
              key={mark.line.key}
              role="slider"
              tabIndex={0}
              className={`lyrics-mark${chosen ? " lyrics-mark-chosen" : ""}`}
              style={{ left: `${left}%` }}
              aria-label={t("lyrics_editor.line_number", { number: mark.number })}
              aria-valuemin={0}
              aria-valuemax={Math.round(length * 1000)}
              aria-valuenow={mark.at}
              aria-valuetext={stampOf(mark.at)}
              title={`${stampOf(mark.at)} · ${mark.line.text || t("lyrics_editor.break")}`}
              onPointerDown={(event) => take(mark.line.key, event)}
              onPointerMove={drag}
              onPointerUp={letGo}
              onPointerCancel={letGo}
              onKeyDown={(event) => keys(mark, event)}
              onKeyUp={() => acts.settle()}
              onFocus={() => acts.select(mark.line.key)}
            >
              <span className="lyrics-mark-words" style={{ maxWidth: `calc(${room}cqw - 8px)` }}>
                <span className="lyrics-mark-number">{mark.number}</span>
                {mark.line.text || "♪"}
              </span>
            </span>
          );
        })}
        {close && (
          <>
            <span className="wave-edge wave-edge-from lyrics-timeline-edge">{clockOf(shown.from)}</span>
            <span className="wave-edge wave-edge-to lyrics-timeline-edge">{clockOf(shown.to)}</span>
          </>
        )}
        <span className="wave-head lyrics-timeline-head" ref={head} />
      </div>
      <WaveOverview wave={wave} length={length} shown={shown} setShown={setShown} zoom={zoom} t={t}>
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
