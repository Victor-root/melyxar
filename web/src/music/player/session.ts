/*
 * What the device shows of the music playing: the card of the lock screen,
 * of the notifications of a phone, of the media keys of a keyboard, and what
 * pressing them does.
 *
 * Stopping takes the card away, which is what a real stop button is for: a
 * paused song keeps its card on a phone for ever, and somebody who has
 * finished listening wants it gone.
 */

import { useEffect } from "react";
import { pictureSet } from "../../api";
import type { Music } from "./player";
import { useMusicTime } from "./player";

export function useMediaSession(music: Music): void {
  const { song, playing } = music;
  const session = typeof navigator !== "undefined" ? navigator.mediaSession : undefined;

  useEffect(() => {
    if (!session) {
      return;
    }
    if (!song) {
      session.metadata = null;
      session.playbackState = "none";
      return;
    }
    const cover = song.cover;
    session.metadata = new MediaMetadata({
      title: song.title,
      artist: song.artists.map((artist) => artist.name).join(", "),
      album: song.album?.name ?? "",
      artwork: cover
        .filter((picture) => picture.width !== null)
        .map((picture) => ({
          src: new URL(picture.url, window.location.origin).toString(),
          sizes: `${picture.width}x${picture.height ?? picture.width}`,
        })),
    });
    // Nothing else in the file names a cover with no size: it goes whole.
    if (cover.length > 0 && cover.every((picture) => picture.width === null)) {
      const whole = pictureSet(cover);
      if (whole) {
        session.metadata.artwork = [{ src: new URL(whole.src, window.location.origin).toString() }];
      }
    }
  }, [session, song]);

  useEffect(() => {
    if (session && song) {
      session.playbackState = playing ? "playing" : "paused";
    }
  }, [session, song, playing]);

  useEffect(() => {
    if (!session) {
      return;
    }
    const handlers: [MediaSessionAction, MediaSessionActionHandler | null][] = [
      ["play", () => !music.playing && music.toggle()],
      ["pause", () => music.playing && music.toggle()],
      ["stop", () => music.stop()],
      ["nexttrack", () => music.next()],
      ["previoustrack", () => music.previous()],
      ["seekto", (details) => details.seekTime !== undefined && music.seek(details.seekTime)],
    ];
    for (const [action, handler] of handlers) {
      try {
        session.setActionHandler(action, handler);
      } catch {
        // A browser that does not know this action shows no button for it.
      }
    }
  }, [session, music]);

}

/**
 * Tells the device where the song has got to. A thing of its own, drawn
 * beside the interface rather than inside the player that holds it: it
 * changes four times a second, and nothing else should be drawn again as
 * often.
 */
export function MediaSessionPosition({ song }: { song: Music["song"] }) {
  const session = typeof navigator !== "undefined" ? navigator.mediaSession : undefined;
  const { position, length } = useMusicTime();
  useEffect(() => {
    if (!session || !song || length <= 0 || position > length) {
      return;
    }
    try {
      session.setPositionState({ duration: length, position, playbackRate: 1 });
    } catch {
      // A position the browser refuses is left for the next one.
    }
  }, [session, song, position, length]);
  return null;
}
