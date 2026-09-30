/*
 * What the server is told of the music playing, for the administration to
 * follow it as it follows the films: which song, where it has got to,
 * whether it is paused, and who is listening on which device.
 *
 * The same words the film player uses, since the server keeps one list of
 * what is playing. Said when a song is loaded, at every change between
 * playing and paused, every few seconds in between, and once more as it is
 * left. A live line is held open beside them for as long as the song is
 * loaded: its end is the player gone, and it is how an administrator's stop
 * arrives.
 */

import { useEffect, useRef } from "react";
import { api } from "../../api";
import type { Song } from "../api";

/** How often the server is told the song is still there. */
const EVERY_SECONDS = 10;

export function useTellTheServer(
  song: Song | null,
  playing: boolean,
  stop: () => void,
  positionNow: () => number,
): void {
  const songId = song?.id ?? null;
  const songNow = useRef(songId);
  songNow.current = songId;
  const playingNow = useRef(playing);
  playingNow.current = playing;
  const stopNow = useRef(stop);
  stopNow.current = stop;
  const positionRead = useRef(positionNow);
  positionRead.current = positionNow;

  const position = () => {
    const seconds = Math.floor(positionRead.current());
    return seconds > 0 ? seconds : null;
  };

  // A song loaded: told at once, and its line opened once it was heard,
  // which is what tells the server this song is on. Told again every few
  // seconds, and as the song is left.
  useEffect(() => {
    if (!songId) {
      return;
    }
    let gone = false;
    let line: EventSource | null = null;
    let inFlight = false;

    const tell = (fresh: boolean) => {
      if (inFlight && !fresh) {
        return;
      }
      inFlight = true;
      api
        .stillPlaying(songId, position(), !playingNow.current, false, fresh)
        .then((answer) => {
          if (gone) {
            return;
          }
          if (answer.stop) {
            stopNow.current();
          }
          if (fresh && !line) {
            line = api.playerLine(songId);
            line.addEventListener("stop", () => stopNow.current());
          }
        })
        .catch(() => {
          // One that did not arrive says nothing: the next carries the same
          // news, and the music keeps playing.
        })
        .finally(() => {
          inFlight = false;
        });
    };

    tell(true);
    const beating = window.setInterval(() => tell(false), EVERY_SECONDS * 1000);
    const onTheWayOut = () => api.stillPlayingOnTheWayOut(songId, position());
    window.addEventListener("pagehide", onTheWayOut);
    return () => {
      gone = true;
      window.clearInterval(beating);
      window.removeEventListener("pagehide", onTheWayOut);
      line?.close();
      api.stillPlaying(songId, position(), true, true).catch(() => {});
    };
  }, [songId]);

  // Paused or played again: the administration sees it at once.
  const first = useRef(true);
  useEffect(() => {
    if (first.current) {
      first.current = false;
      return;
    }
    if (songNow.current) {
      api.stillPlaying(songNow.current, position(), !playing).catch(() => {});
    }
    // Only a change between playing and paused is worth saying at once.
  }, [playing]);
}
