/*
 * The wave behind the bar of the player: the levels the server kept for the
 * song, read at the place the song has got to and drawn as one smooth wave
 * rising from the foot of the bar, in the accent. It only dresses the bar, so
 * it costs as little as drawing can: thirty drawings a second at most, and
 * nothing runs while the song is paused, once the wave has settled, while
 * the song has no levels, or while the tab is hidden.
 */

import { useEffect, useRef, useState } from "react";
import { music as server } from "../api";
import type { SongSpectrum } from "../api";
import { useMusic, useMusicTime } from "./player";
import { approach, levelsAt, momentOf } from "./spectrum-data";

const FRAME_EVERY_MS = 33;

/** The levels kept for a song, nothing until they have come or when it has
 *  none. */
function useSongSpectrum(song: string): SongSpectrum | null {
  const [found, setFound] = useState<{ song: string; spectrum: SongSpectrum | null } | null>(null);
  useEffect(() => {
    const controller = new AbortController();
    server.spectrum(song, controller.signal).then(
      (spectrum) => setFound({ song, spectrum }),
      () => {
        if (!controller.signal.aborted) {
          setFound({ song, spectrum: null });
        }
      },
    );
    return () => controller.abort();
  }, [song]);
  return found?.song === song ? found.spectrum : null;
}

/** The wave behind a player, when this account wants one and the song is
 *  there to draw it for. Held still, as it was last drawn, while something
 *  covers it. */
export function BehindThePlayer({ still = false }: { still?: boolean }) {
  const { song, playing, preferences } = useMusic();
  if (!song || !preferences.spectrum) {
    return null;
  }
  return <Spectrum song={song.id} playing={playing} still={still} amplitude={preferences.spectrum_amplitude / 100} />;
}

function Spectrum({
  song,
  playing,
  still,
  amplitude,
}: {
  song: string;
  playing: boolean;
  still: boolean;
  amplitude: number;
}) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const spectrum = useSongSpectrum(song);
  const { position } = useMusicTime();
  /* When the player last said where the song was. */
  const said = useRef({ position, at: performance.now() });
  useEffect(() => {
    said.current = { position, at: performance.now() };
  }, [position, playing]);
  /* Read as it is drawn, so that moving the slider changes the wave at once
     and does not start the drawing over. */
  const amplitudeNow = useRef(amplitude);
  amplitudeNow.current = amplitude;
  /* Kept across a pause and a change of song, so the wave glides rather than
     drops. */
  const kept = useRef<number[]>([]);

  useEffect(() => {
    const element = canvas.current;
    const context = element?.getContext("2d");
    if (!element || !context) {
      return;
    }
    const levels = kept.current;
    if (spectrum && levels.length !== spectrum.bands) {
      levels.length = spectrum.bands;
      levels.fill(0);
    }
    const targets = new Array<number>(levels.length).fill(0);
    let color = getComputedStyle(element).color;
    let width = 0;
    let height = 0;
    let frame = 0;
    let lastDrawn = 0;

    const draw = () => {
      context.clearRect(0, 0, width, height);
      if (levels.length < 2) {
        return;
      }
      context.fillStyle = color;
      const step = width / (levels.length - 1);
      const top = (band: number) => height - levels[band] * amplitudeNow.current * height;
      context.beginPath();
      context.moveTo(0, height);
      context.lineTo(0, top(0));
      for (let band = 0; band < levels.length - 1; band += 1) {
        context.quadraticCurveTo(band * step, top(band), (band + 0.5) * step, (top(band) + top(band + 1)) / 2);
      }
      context.lineTo(width, top(levels.length - 1));
      context.lineTo(width, height);
      context.closePath();
      context.fill();
    };

    const tick = (now: number) => {
      frame = 0;
      if (document.hidden) {
        return;
      }
      if (now - lastDrawn >= FRAME_EVERY_MS) {
        lastDrawn = now;
        const reading = playing && spectrum !== null;
        if (reading) {
          levelsAt(spectrum, momentOf(said.current.position, now - said.current.at), targets);
        } else {
          targets.fill(0);
        }
        const settled = approach(levels, targets);
        draw();
        if (!reading && settled) {
          return;
        }
      }
      frame = requestAnimationFrame(tick);
    };

    const start = () => {
      if (!frame && !document.hidden && !still) {
        frame = requestAnimationFrame(tick);
      }
    };

    const watcher = new ResizeObserver(([entry]) => {
      width = Math.round(entry.contentRect.width);
      height = Math.round(entry.contentRect.height);
      element.width = width;
      element.height = height;
      draw();
    });
    watcher.observe(element);
    // The accent and the theme are set on the root, and the wave follows them
    // as they change rather than as it is next started.
    const colours = new MutationObserver(() => {
      color = getComputedStyle(element).color;
    });
    colours.observe(document.documentElement, { attributeFilter: ["style", "data-theme"] });
    document.addEventListener("visibilitychange", start);
    start();
    return () => {
      watcher.disconnect();
      colours.disconnect();
      document.removeEventListener("visibilitychange", start);
      cancelAnimationFrame(frame);
    };
  }, [playing, spectrum, still]);

  return <canvas ref={canvas} className="music-bar-spectrum" aria-hidden="true" />;
}
