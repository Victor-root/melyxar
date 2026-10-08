/*
 * A video of one's own run through under the pointer: its card shows, one
 * after another, pictures from all along it, cut out of the sheets the bar of
 * the player already uses. Nothing is asked of the server, and nothing is
 * drawn, until a pointer rests on the card.
 */

import { useEffect, useRef, useState } from "react";
import { api } from "../api";
import type { PlaybackThumbnails } from "../api";
import { cutOut, previewMoments, sheetUrlOf, sheetUrls } from "../player/thumbnail";

/** How many pictures a card runs through. */
const PICTURES = 8;

/** How long each one stays. */
const EACH_FOR_MS = 700;

/** How long a pointer rests on a card before it starts: one passing over a
 *  grid on its way somewhere else starts nothing. */
const RESTING_MS = 350;

/** The thumbnails of each copy, asked for once. A copy not read for them yet
 *  is asked again next time, since the scan may have read it since. */
const known = new Map<string, Promise<PlaybackThumbnails | null>>();

function thumbnailsOf(source: string): Promise<PlaybackThumbnails | null> {
  let asked = known.get(source);
  if (!asked) {
    asked = api.thumbnailsOf(source).catch(() => {
      known.delete(source);
      return null;
    });
    known.set(source, asked);
  }
  return asked;
}

/** Whether a card is running through its pictures, and the two moments that
 *  start and stop it. */
export function useHoverPreview(wanted: boolean): { on: boolean; begin: () => void; end: () => void } {
  const [on, setOn] = useState(false);
  const resting = useRef<number | undefined>(undefined);
  useEffect(() => () => window.clearTimeout(resting.current), []);
  return {
    on,
    begin: () => {
      if (!wanted) {
        return;
      }
      window.clearTimeout(resting.current);
      resting.current = window.setTimeout(() => setOn(true), RESTING_MS);
    },
    end: () => {
      window.clearTimeout(resting.current);
      setOn(false);
    },
  };
}

/** The pictures of a video, one after another over its card. Each is shown
 *  only once its sheet has arrived, so the card never flashes empty. */
export function RunningPreview({ source }: { source: string }) {
  const [thumbnails, setThumbnails] = useState<PlaybackThumbnails | null>(null);
  const [arrived, setArrived] = useState<ReadonlySet<string>>(new Set());
  const [at, setAt] = useState(0);

  useEffect(() => {
    let current = true;
    void thumbnailsOf(source).then((read) => current && setThumbnails(read));
    return () => {
      current = false;
    };
  }, [source]);

  const moments = thumbnails ? previewMoments(thumbnails, PICTURES) : [];

  useEffect(() => {
    if (!thumbnails) {
      return;
    }
    let current = true;
    for (const url of sheetUrls(thumbnails, previewMoments(thumbnails, PICTURES))) {
      const sheet = new Image();
      sheet.onload = () => current && setArrived((was) => new Set(was).add(url));
      sheet.src = url;
    }
    return () => {
      current = false;
    };
  }, [thumbnails]);

  useEffect(() => {
    const turning = window.setInterval(() => setAt((was) => was + 1), EACH_FOR_MS);
    return () => window.clearInterval(turning);
  }, []);

  if (!thumbnails) {
    return null;
  }
  const ready = moments.filter((moment) => {
    const url = sheetUrlOf(thumbnails, moment);
    return url !== null && arrived.has(url);
  });
  if (ready.length === 0) {
    return null;
  }
  const picture = cutOut(thumbnails, ready[at % ready.length]);
  return picture && <span className="card-preview" style={picture} aria-hidden="true" />;
}
