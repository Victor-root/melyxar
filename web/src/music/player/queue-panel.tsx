/*
 * The queue as the page of what is playing shows it: a line for every song,
 * as a list of songs draws it, with a button that plays or pauses it, the
 * little wave on the one playing, and at the right a grip to put the line
 * elsewhere in the order.
 *
 * A line is moved as on a rail: it follows the pointer up and down and
 * nowhere else, the others make room as it passes them, and the order changes
 * when it is let go. The arrow keys on the grip move it a place at a time.
 */

import { memo, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { CSSProperties, KeyboardEvent, PointerEvent } from "react";
import { asClock } from "../../clock";
import { CloseIcon, GripIcon, PlayIcon } from "../../icons";
import { scrollerOf } from "../../landing";
import { useSettings } from "../../settings";
import { PlayingBars } from "../playing-bars";
import { Cover } from "../songs";
import { keeping, linesAround, linesShown } from "./lines-shown";
import type { Shown } from "./lines-shown";
import type { Song } from "../api";
import { useMusic } from "./player";

/** How far from the top or foot of the list the pointer sets it scrolling. */
const SCROLL_EDGE = 56;

/** How far a finger on the grip goes before it is told which way it means to
 *  go: up or down moves the line, sideways is a swipe from one page to the
 *  next, which the grip must let through. */
const MEANT_TO_MOVE = 6;

/** A finger that is on a grip and has not yet said what it means to do. */
interface Pending {
  at: number;
  grip: HTMLElement;
  pointerId: number;
  x: number;
  y: number;
}

/** A line being moved, from where it was to where it would land. */
interface Drag {
  from: number;
  to: number;
}

/** What the pointer has done since a line was taken. */
interface Taken {
  from: number;
  to: number;
  line: HTMLElement;
  height: number;
  count: number;
  startY: number;
  pointerY: number;
  startScroll: number;
  frame: number;
}

const between = (value: number, least: number, most: number) => Math.min(Math.max(value, least), most);

export function QueuePanel() {
  const music = useMusic();
  const { queue, playing } = music;
  const list = useRef<HTMLOListElement>(null);
  const taken = useRef<Taken | null>(null);
  const pending = useRef<Pending | null>(null);
  const [drag, setDrag] = useState<Drag | null>(null);
  const count = queue.order.length;

  /* Only the lines in sight and a margin around them are made, the rest of
     the list being room kept above and below them: a queue of a whole
     library is two thousand songs. Every line is as tall as the others. */
  const [line, setLine] = useState(0);
  const [made, setMade] = useState<Shown>(() => linesAround(queue.at, count));
  const scroller = useRef<HTMLElement | null>(null);
  /* How far down the box the list begins, read when either changes size. */
  const listTop = useRef(0);
  const shown = keeping({ from: Math.min(made.from, count), to: Math.min(made.to, count) }, drag?.from ?? null);

  const placeOfTheList = () => {
    const element = list.current;
    const box = scroller.current;
    if (element && box) {
      listTop.current =
        box === element ? 0 : element.getBoundingClientRect().top - box.getBoundingClientRect().top + box.scrollTop;
    }
  };
  const refresh = useRef(() => {});
  refresh.current = () => {
    const box = scroller.current;
    if (box && line > 0) {
      const next = linesShown(box.scrollTop, box.clientHeight, listTop.current, line, count);
      setMade((was) => (was.from === next.from && was.to === next.to ? was : next));
    }
  };

  /* Where the box stands to put the song playing in its middle. */
  const middle = (box: HTMLElement, height: number) => listTop.current + queue.at * height - (box.clientHeight - height) / 2;

  /* Everything measured at once when the queue opens, in one reading of the
     page: the box, where the list begins, how tall a line is, and from them
     the lines to make around the song playing. */
  useLayoutEffect(() => {
    const first = list.current?.firstElementChild as HTMLElement | null | undefined;
    if (line > 0 || !first) {
      return;
    }
    const box = scrollerOf(first);
    scroller.current = box;
    placeOfTheList();
    const height = first.getBoundingClientRect().height;
    if (box) {
      setMade(linesShown(middle(box, height), box.clientHeight, listTop.current, height, count));
    }
    setLine(height);
  });

  useEffect(() => {
    const element = list.current;
    const box = scroller.current;
    if (line === 0 || !element || !box) {
      return;
    }
    let frame = 0;
    let moved = false;
    const scrolled = () => {
      if (frame === 0) {
        frame = requestAnimationFrame(() => {
          frame = 0;
          if (moved) {
            moved = false;
            placeOfTheList();
          }
          refresh.current();
        });
      }
    };
    /* Read on the next frame with the scroll, not as the watcher reports:
       other watchers of the page change it at that moment, and reading it
       then laid it out again for nothing. */
    const resized = new ResizeObserver(() => {
      moved = true;
      scrolled();
    });
    resized.observe(element);
    resized.observe(box);
    box.addEventListener("scroll", scrolled, { passive: true });
    return () => {
      cancelAnimationFrame(frame);
      resized.disconnect();
      box.removeEventListener("scroll", scrolled);
    };
  }, [line]);

  useEffect(() => refresh.current(), [count]);

  /* The song playing in the middle of the box the list scrolls in: put there
     before anything is drawn when the queue opens, so that it never opens on
     the first lines of a long queue, and glided to when the song changes. */
  const centre = (behavior: ScrollBehavior) => {
    const box = scroller.current;
    if (box && line > 0) {
      placeOfTheList();
      box.scrollTo({ top: listTop.current + queue.at * line - (box.clientHeight - line) / 2, behavior });
    }
  };
  const measured = line > 0;
  useLayoutEffect(() => {
    const box = scroller.current;
    if (!measured || !box) {
      return;
    }
    box.scrollTo({ top: middle(box, line), behavior: "instant" });
    /* Again a frame or two later, once the page around it has taken its
       room: the foot of the page is measured after the first drawing, and
       until it has its room there is not enough below the last lines to
       bring them to the middle. */
    let frame = requestAnimationFrame(() => {
      frame = requestAnimationFrame(() => centre("instant"));
    });
    return () => cancelAnimationFrame(frame);
  }, [measured]);
  const first = useRef(true);
  useEffect(() => {
    if (first.current) {
      first.current = false;
      return;
    }
    centre("smooth");
  }, [queue.at]);

  /* Run for as long as a line is held: the line goes where the pointer is,
     in height only, the list scrolls when the pointer is at its edge, and the
     place the line would take is worked out from how far it has gone. */
  const follow = () => {
    const held = taken.current;
    const box = list.current;
    if (!held || !box) {
      return;
    }
    const edge = box.getBoundingClientRect();
    if (held.pointerY < edge.top + SCROLL_EDGE) {
      box.scrollTop -= Math.ceil((edge.top + SCROLL_EDGE - held.pointerY) / 4);
    } else if (held.pointerY > edge.bottom - SCROLL_EDGE) {
      box.scrollTop += Math.ceil((held.pointerY - (edge.bottom - SCROLL_EDGE)) / 4);
    }
    const moved = between(
      held.pointerY - held.startY + box.scrollTop - held.startScroll,
      -held.from * held.height,
      (held.count - 1 - held.from) * held.height,
    );
    held.line.style.transform = `translateY(${moved}px)`;
    const to = between(Math.round(held.from + moved / held.height), 0, held.count - 1);
    if (to !== held.to) {
      held.to = to;
      setDrag({ from: held.from, to });
    }
    held.frame = requestAnimationFrame(follow);
  };

  const take = (grip: HTMLElement, pointerId: number, clientY: number, at: number) => {
    const box = list.current;
    const line = grip.closest("li");
    if (!box || !line) {
      return;
    }
    grip.setPointerCapture(pointerId);
    const height = line.getBoundingClientRect().height;
    box.style.setProperty("--queue-line", `${height}px`);
    taken.current = {
      from: at,
      to: at,
      line,
      height,
      count: queue.order.length,
      startY: clientY,
      pointerY: clientY,
      startScroll: box.scrollTop,
      frame: requestAnimationFrame(follow),
    };
    setDrag({ from: at, to: at });
  };

  /* A mouse takes a line at once. A finger first says which way it goes: a
     finger that lands on a grip while swiping from one page to another meant
     the swipe, and must not be taken for a line being moved. */
  const press = (event: PointerEvent<HTMLButtonElement>, at: number) => {
    if (event.button !== 0) {
      return;
    }
    if (event.pointerType === "mouse") {
      take(event.currentTarget, event.pointerId, event.clientY, at);
      return;
    }
    pending.current = { at, grip: event.currentTarget, pointerId: event.pointerId, x: event.clientX, y: event.clientY };
  };

  const moved = (event: PointerEvent<HTMLButtonElement>) => {
    const waiting = pending.current;
    if (waiting) {
      const across = Math.abs(event.clientX - waiting.x);
      const down = Math.abs(event.clientY - waiting.y);
      if (down > MEANT_TO_MOVE && down > across) {
        pending.current = null;
        take(waiting.grip, waiting.pointerId, waiting.y, waiting.at);
        if (taken.current) {
          taken.current.pointerY = event.clientY;
        }
      } else if (across > MEANT_TO_MOVE) {
        pending.current = null;
      }
    } else if (taken.current) {
      taken.current.pointerY = event.clientY;
    }
  };

  const release = (landed: boolean) => {
    pending.current = null;
    const held = taken.current;
    if (!held) {
      return;
    }
    cancelAnimationFrame(held.frame);
    held.line.style.transform = "";
    taken.current = null;
    setDrag(null);
    if (landed && held.to !== held.from) {
      music.move(held.from, held.to);
    }
  };

  const step = (event: KeyboardEvent<HTMLButtonElement>, at: number) => {
    const to = event.key === "ArrowUp" ? at - 1 : event.key === "ArrowDown" ? at + 1 : null;
    if (to === null) {
      return;
    }
    event.preventDefault();
    music.move(at, to);
  };

  /* What a line's buttons do, the same from one drawing to the next and read
     when pressed: a line is then drawn again only when what it shows changes,
     not each time the lines made move along with the scroll. */
  const doing = useRef({ music, press, moved, release, step });
  doing.current = { music, press, moved, release, step };
  const hands = useMemo<Hands>(
    () => ({
      play: (at, here) => (here ? doing.current.music.toggle() : doing.current.music.jump(at)),
      remove: (at) => doing.current.music.remove(at),
      step: (event, at) => doing.current.step(event, at),
      press: (event, at) => doing.current.press(event, at),
      moved: (event) => doing.current.moved(event),
      release: (landed) => doing.current.release(landed),
    }),
    [],
  );

  return (
    <ol
      ref={list}
      className={`music-queue${drag ? " music-queue-sorting" : ""}`}
      style={
        { "--queue-room-before": `${shown.from * line}px`, "--queue-room-after": `${(count - shown.to) * line}px` } as CSSProperties
      }
    >
      {queue.order.slice(shown.from, shown.to).map((place, index) => {
        const at = shown.from + index;
        const here = at === queue.at;
        /* The lines the one held passes make room for it. */
        const makesRoom =
          drag && drag.from < drag.to && at > drag.from && at <= drag.to
            ? " music-queue-room-before"
            : drag && drag.from > drag.to && at >= drag.to && at < drag.from
              ? " music-queue-room-after"
              : "";
        return (
          <QueueLine
            key={place}
            at={at}
            song={queue.songs[place]}
            here={here}
            playing={here && playing}
            played={at < queue.at}
            removable={at > queue.at}
            lifted={drag?.from === at}
            makesRoom={makesRoom}
            hands={hands}
          />
        );
      })}
    </ol>
  );
}

/** What the lines of the queue do when pressed. */
interface Hands {
  play: (at: number, here: boolean) => void;
  remove: (at: number) => void;
  step: (event: KeyboardEvent<HTMLButtonElement>, at: number) => void;
  press: (event: PointerEvent<HTMLButtonElement>, at: number) => void;
  moved: (event: PointerEvent<HTMLButtonElement>) => void;
  release: (landed: boolean) => void;
}

const QueueLine = memo(function QueueLine({
  at,
  song,
  here,
  playing,
  played,
  removable,
  lifted,
  makesRoom,
  hands,
}: {
  at: number;
  song: Song;
  here: boolean;
  playing: boolean;
  played: boolean;
  removable: boolean;
  lifted: boolean;
  makesRoom: string;
  hands: Hands;
}) {
  const { t } = useSettings();
  const label = here ? t(playing ? "music.pause" : "music.play") : t("music.play_song", { title: song.title });
  const play = () => hands.play(at, here);
  return (
    <li
      className={`music-song music-song-full music-queue-line${here ? " music-song-playing" : ""}${played ? " music-queue-played" : ""}${lifted ? " music-queue-lifted" : ""}${makesRoom}`}
    >
      <span className="music-song-lead">
        <button type="button" className="music-song-toggle" aria-label={label} title={label} onClick={play}>
          {playing ? <PlayingBars /> : <PlayIcon size={20} />}
        </button>
      </span>
      <span className="music-song-number">{at + 1}</span>
      <Cover song={song} />
      <button type="button" className="music-queue-song" onClick={play} aria-current={here ? "true" : undefined}>
        <span className="music-song-title">{song.title}</span>
        <span className="music-song-artists">{song.artists.map((artist) => artist.name).join(", ")}</span>
      </button>
      <span className="music-song-length">{song.seconds === null ? "" : asClock(song.seconds)}</span>
      {removable ? (
        <button
          type="button"
          className="player-button player-button-small music-queue-remove"
          onClick={() => hands.remove(at)}
          aria-label={t("music.take_out", { title: song.title })}
          title={t("music.take_out", { title: song.title })}
        >
          <CloseIcon size={14} />
        </button>
      ) : (
        <span className="music-queue-remove" />
      )}
      <button
        type="button"
        className="music-queue-grip"
        aria-label={t("music.move_song", { title: song.title })}
        title={t("music.move_song", { title: song.title })}
        onKeyDown={(event) => hands.step(event, at)}
        onPointerDown={(event) => hands.press(event, at)}
        onPointerMove={hands.moved}
        onPointerUp={() => hands.release(true)}
        onPointerCancel={() => hands.release(false)}
      >
        <GripIcon size={18} />
      </button>
    </li>
  );
});
