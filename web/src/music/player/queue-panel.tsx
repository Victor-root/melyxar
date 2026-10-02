/*
 * The queue as the page of what is playing shows it: a line for every song,
 * with its cover, a button that plays or pauses it, the little wave on the
 * one playing, and at the right a grip to take hold of to put it elsewhere
 * in the order. A line is dragged by its grip, or moved a place by the arrow
 * keys on it.
 */

import { useState } from "react";
import type { DragEvent, KeyboardEvent } from "react";
import { asClock } from "../../clock";
import { CloseIcon, GripIcon, PlayIcon } from "../../icons";
import { PauseIcon } from "../../player/icons";
import { useSettings } from "../../settings";
import { Cover } from "../songs";
import { PlayingWave } from "../playing-wave";
import { useMusic } from "./player";

/** Where a song being dragged would go: the place in the order it would take
 *  once dropped, and which line the mark of it is drawn beside. */
interface Drag {
  from: number;
  to: number;
}

export function QueuePanel() {
  const { t } = useSettings();
  const music = useMusic();
  const { queue, playing } = music;
  const [drag, setDrag] = useState<Drag | null>(null);

  /* Over a line, the song goes before it when the pointer is in its upper
     half and after it otherwise. */
  const over = (event: DragEvent<HTMLOListElement>) => {
    const line = (event.target as HTMLElement).closest<HTMLElement>("li[data-at]");
    if (!drag || !line) {
      return;
    }
    event.preventDefault();
    const at = Number(line.dataset.at);
    const box = line.getBoundingClientRect();
    const slot = event.clientY > box.top + box.height / 2 ? at + 1 : at;
    const to = slot > drag.from ? slot - 1 : slot;
    if (to !== drag.to) {
      setDrag({ from: drag.from, to });
    }
  };

  const drop = (event: DragEvent<HTMLOListElement>) => {
    event.preventDefault();
    if (drag) {
      music.move(drag.from, drag.to);
    }
    setDrag(null);
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
    <ol className="music-queue" onDragOver={over} onDrop={drop}>
      {queue.order.map((place, at) => {
        const one = queue.songs[place];
        const here = at === queue.at;
        const label = here ? t(playing ? "music.pause" : "music.play") : t("music.play_song", { title: one.title });
        const press = () => (here ? music.toggle() : music.jump(at));
        const mark =
          drag && drag.to !== drag.from && drag.to === at
            ? drag.to > drag.from
              ? " music-queue-drop-after"
              : " music-queue-drop-before"
            : "";
        return (
          <li
            key={`${place}-${at}`}
            data-at={at}
            className={`music-queue-line${here ? " music-queue-here music-song-playing" : ""}${at < queue.at ? " music-queue-played" : ""}${drag?.from === at ? " music-queue-lifted" : ""}${mark}`}
          >
            <button type="button" className="music-song-toggle" aria-label={label} title={label} onClick={press}>
              {here && playing ? <PauseIcon size={20} /> : <PlayIcon size={20} />}
            </button>
            <span className="music-song-number">{here ? <PlayingWave playing={playing} /> : at + 1}</span>
            <Cover song={one} />
            <button type="button" className="music-queue-song" onClick={press} aria-current={here ? "true" : undefined}>
              <span className="music-queue-title">{one.title}</span>
              <span className="music-queue-artists">{one.artists.map((artist) => artist.name).join(", ")}</span>
            </button>
            <span className="music-queue-length">{one.seconds === null ? "" : asClock(one.seconds)}</span>
            {at > queue.at ? (
              <button
                type="button"
                className="player-button player-button-small music-queue-remove"
                onClick={() => music.remove(at)}
                aria-label={t("music.take_out", { title: one.title })}
              >
                <CloseIcon size={14} />
              </button>
            ) : (
              <span className="music-queue-remove" />
            )}
            <button
              type="button"
              className="music-queue-grip"
              draggable
              aria-label={t("music.move_song", { title: one.title })}
              title={t("music.move_song", { title: one.title })}
              onKeyDown={(event) => step(event, at)}
              onDragStart={(event) => {
                const line = event.currentTarget.closest("li");
                event.dataTransfer.effectAllowed = "move";
                event.dataTransfer.setData("text/plain", String(at));
                if (line) {
                  event.dataTransfer.setDragImage(line, 24, 24);
                }
                setDrag({ from: at, to: at });
              }}
              onDragEnd={() => setDrag(null)}
            >
              <GripIcon size={18} />
            </button>
          </li>
        );
      })}
    </ol>
  );
}
