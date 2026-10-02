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

import { useRef, useState } from "react";
import type { KeyboardEvent, PointerEvent } from "react";
import { asClock } from "../../clock";
import { CloseIcon, GripIcon, PlayIcon } from "../../icons";
import { PauseIcon } from "../../player/icons";
import { useSettings } from "../../settings";
import { PlayingWave } from "../playing-wave";
import { Cover } from "../songs";
import { useMusic } from "./player";

/** How far from the top or foot of the list the pointer sets it scrolling. */
const SCROLL_EDGE = 56;

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
  const { t } = useSettings();
  const music = useMusic();
  const { queue, playing } = music;
  const list = useRef<HTMLOListElement>(null);
  const taken = useRef<Taken | null>(null);
  const [drag, setDrag] = useState<Drag | null>(null);

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

  const take = (event: PointerEvent<HTMLButtonElement>, at: number) => {
    const box = list.current;
    const line = event.currentTarget.closest("li");
    if (event.button !== 0 || !box || !line) {
      return;
    }
    event.currentTarget.setPointerCapture(event.pointerId);
    const height = line.getBoundingClientRect().height;
    box.style.setProperty("--queue-line", `${height}px`);
    taken.current = {
      from: at,
      to: at,
      line,
      height,
      count: queue.order.length,
      startY: event.clientY,
      pointerY: event.clientY,
      startScroll: box.scrollTop,
      frame: requestAnimationFrame(follow),
    };
    setDrag({ from: at, to: at });
  };

  const release = (landed: boolean) => {
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

  return (
    <ol ref={list} className={`music-queue${drag ? " music-queue-sorting" : ""}`}>
      {queue.order.map((place, at) => {
        const one = queue.songs[place];
        const here = at === queue.at;
        const label = here ? t(playing ? "music.pause" : "music.play") : t("music.play_song", { title: one.title });
        const press = () => (here ? music.toggle() : music.jump(at));
        /* The lines the one held passes make room for it. */
        const makesRoom =
          drag && drag.from < drag.to && at > drag.from && at <= drag.to
            ? " music-queue-room-before"
            : drag && drag.from > drag.to && at >= drag.to && at < drag.from
              ? " music-queue-room-after"
              : "";
        return (
          <li
            key={place}
            className={`music-song music-song-full music-queue-line${here ? " music-song-playing" : ""}${at < queue.at ? " music-queue-played" : ""}${drag?.from === at ? " music-queue-lifted" : ""}${makesRoom}`}
          >
            <span className="music-song-lead">
              <button type="button" className="music-song-toggle" aria-label={label} title={label} onClick={press}>
                {here && playing ? <PauseIcon size={20} /> : <PlayIcon size={20} />}
              </button>
            </span>
            <span className="music-song-number">{here ? <PlayingWave playing={playing} /> : at + 1}</span>
            <Cover song={one} />
            <button type="button" className="music-queue-song" onClick={press} aria-current={here ? "true" : undefined}>
              <span className="music-song-title">{one.title}</span>
              <span className="music-song-artists">{one.artists.map((artist) => artist.name).join(", ")}</span>
            </button>
            <span className="music-song-length">{one.seconds === null ? "" : asClock(one.seconds)}</span>
            {at > queue.at ? (
              <button
                type="button"
                className="player-button player-button-small music-queue-remove"
                onClick={() => music.remove(at)}
                aria-label={t("music.take_out", { title: one.title })}
                title={t("music.take_out", { title: one.title })}
              >
                <CloseIcon size={14} />
              </button>
            ) : (
              <span className="music-queue-remove" />
            )}
            <button
              type="button"
              className="music-queue-grip"
              aria-label={t("music.move_song", { title: one.title })}
              title={t("music.move_song", { title: one.title })}
              onKeyDown={(event) => step(event, at)}
              onPointerDown={(event) => take(event, at)}
              onPointerMove={(event) => {
                if (taken.current) {
                  taken.current.pointerY = event.clientY;
                }
              }}
              onPointerUp={() => release(true)}
              onPointerCancel={() => release(false)}
            >
              <GripIcon size={18} />
            </button>
          </li>
        );
      })}
    </ol>
  );
}
