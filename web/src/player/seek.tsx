/*
 * The bar a film is moved along: how far it has played and how much is ready,
 * the handle, and the little picture of wherever a hand is over it.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import type { PlaybackThumbnails } from "../api";
import { asClock } from "../clock";
import type { Playback } from "./engine";
import type { Turn } from "./panels";
import type { Wording } from "../readable";
import { Thumbnail } from "./thumbnail";

/**
 * How wide the little picture over the bar is drawn, and where its middle
 * stands along the bar, for a hand at `share` of the way along it.
 *
 * Never wider than the bar it stands on. A viewer can make these larger, and a
 * window can be made narrower than the largest of them: past that point it is
 * the bar that decides, because a picture wider than the bar cannot be kept
 * inside the screen at both ends whatever it is centred on. And kept inside
 * the bar at both ends rather than half off the screen.
 */
export function previewPlace(
  wanted: number,
  share: number,
  railWidth: number,
): { across: number; left: number } {
  const across = railWidth > 0 ? Math.min(wanted, railWidth) : wanted;
  const half = across / 2;
  const left = Math.min(Math.max(share * railWidth, half), Math.max(half, railWidth - half));
  return { across, left };
}

/** The bar, its ends, and the little picture above it. */
export function Seek({
  playback,
  thumbnails,
  previewScale,
  turn,
  before,
  after,
  t,
}: {
  playback: Playback;
  thumbnails: PlaybackThumbnails | null;
  /** How much larger than they were made the little pictures are shown. */
  previewScale: number;
  turn: Turn;
  /** What the arrangement puts at either end of the bar. */
  before: ReactNode;
  after: ReactNode;
  t: Wording;
}) {
  const { at, length, loaded } = playback;
  const rail = useRef<HTMLDivElement>(null);
  /* Where the cursor is along the bar, from nought to one, while it is on it.
     Null the rest of the time, which is what hides the preview. */
  const [hovered, setHovered] = useState<number | null>(null);
  const [railWidth, setRailWidth] = useState(0);
  const [dragging, setDragging] = useState(false);
  /* Whether the hand went anywhere between landing on the bar and coming off
     it. A click and a drag leave the film in the same place and are not the
     same gesture: only this knows which happened. */
  const wasDragged = useRef(false);

  const shareAt = useCallback((clientX: number): number | null => {
    const bar = rail.current?.getBoundingClientRect();
    if (!bar || bar.width <= 0) {
      return null;
    }
    setRailWidth(bar.width);
    return Math.min(1, Math.max(0, (clientX - bar.left) / bar.width));
  }, []);

  const goToShare = useCallback(
    (share: number) => {
      if (length > 0) {
        playback.goTo(share * length);
      }
    },
    [playback, length],
  );

  /* Followed on the window rather than on the bar: a finger that leaves the
     bar while still held down is still dragging, and a bar that stops
     following it there is a bar that jumps back. */
  useEffect(() => {
    if (!dragging) {
      return;
    }
    const moved = (event: PointerEvent) => {
      const share = shareAt(event.clientX);
      if (share !== null) {
        wasDragged.current = true;
        setHovered(share);
        goToShare(share);
      }
    };
    const letGo = () => {
      setDragging(false);
      // The hand let go somewhere on the window, not necessarily back over
      // the bar: nothing else is left to tell the preview to go, since the
      // one thing that usually does, leaving the bar, may already have
      // happened once mid-drag and answered to nothing while it was one.
      setHovered(null);
      playback.viewerMoved(wasDragged.current ? "a_drag" : "a_click");
    };
    window.addEventListener("pointermove", moved);
    window.addEventListener("pointerup", letGo);
    window.addEventListener("pointercancel", letGo);
    return () => {
      window.removeEventListener("pointermove", moved);
      window.removeEventListener("pointerup", letGo);
      window.removeEventListener("pointercancel", letGo);
    };
  }, [dragging, shareAt, goToShare, playback]);

  const played = length > 0 ? Math.min(1, at / length) : 0;
  const held = length > 0 ? Math.min(1, loaded / length) : 0;
  const previewed = hovered !== null && length > 0 ? hovered * length : null;
  const { across, left: previewLeft } = previewPlace(
    (thumbnails?.width ?? 0) * previewScale,
    hovered ?? 0,
    railWidth,
  );

  return (
    <div className="player-seek">
      {/* The two ends of the bar hold whatever the arrangement puts there and
          are sized by it, never by a width written here: a box wider than its
          own words pushes the bar away from one end and not the other, and the
          bar stops being centred between the two. */}
      <span className="player-seek-end">
        {before}
      </span>

      <div
        className="player-rail"
        ref={rail}
        role="slider"
        tabIndex={0}
        aria-label={t("player.position")}
        aria-valuemin={0}
        aria-valuemax={Math.round(length)}
        aria-valuenow={Math.round(at)}
        aria-valuetext={asClock(at)}
        onPointerDown={(event) => {
          const share = shareAt(event.clientX);
          if (share !== null) {
            wasDragged.current = false;
            playback.viewerMoving();
            setDragging(true);
            setHovered(share);
            goToShare(share);
          }
        }}
        onPointerMove={(event) => setHovered(shareAt(event.clientX))}
        onPointerLeave={() => {
          if (!dragging) {
            setHovered(null);
          }
        }}
      >
        {/* Inside the bar rather than beside it, because where it stands is a
            place along the bar. Hung on the row instead, it was out by the
            width of the clock to its left: it followed the hand correctly and
            sat beside it the whole way. */}
        {previewed !== null && (
          <div className="player-preview" style={{ left: `${previewLeft}px` }} aria-hidden="true">
            <Thumbnail
              thumbnails={thumbnails}
              seconds={previewed}
              across={across}
              className="player-preview-picture"
              turn={turn}
            />
            {/* Under the picture rather than written across it: a time on top
                of a dark frame of film is a time nobody can read. */}
            <span className="player-preview-time">{asClock(previewed)}</span>
          </div>
        )}
        <span className="player-rail-fill">
          <span className="player-rail-track" />
          <span className="player-rail-held" style={{ width: `${held * 100}%` }} />
          <span className="player-rail-played" style={{ width: `${played * 100}%` }} />
        </span>
        <span className="player-rail-handle" style={{ left: `${played * 100}%` }} />
      </div>

      <span className="player-seek-end">
        {after}
      </span>
    </div>
  );
}
