/*
 * The account's other devices, as a remote steers them: which can be
 * reached and what plays there, told by the page's live line the moment it
 * changes, and where what plays has got to by now.
 */

import { useEffect, useState } from "react";
import type { SteeredPlaying, SteerableDevice } from "../api";
import { useDevicesNews } from "../live";

/** The devices as last told, when they were told, and whether they could not
 *  be had. */
export interface Devices {
  devices: SteerableDevice[] | null;
  heardAt: number;
  cut: boolean;
}

/** Follows the account's other devices for as long as the caller is on the
 *  page. */
export function useSteerableDevices(): Devices {
  const [held, setHeld] = useState<Devices>({ devices: null, heardAt: 0, cut: false });
  useDevicesNews((news) =>
    setHeld((was) =>
      "cut" in news
        ? { ...was, cut: true }
        : { devices: news.devices, heardAt: news.heardAt, cut: false },
    ),
  );
  return held;
}

/** Where what plays has got to at `now`: where it was when told, plus the
 *  time since while it plays, never past its end. */
export function positionNow(playing: SteeredPlaying, heardAt: number, now: number): number {
  const ran = playing.paused ? 0 : Math.max(0, now - heardAt) / 1000;
  const reached = playing.position_seconds + ran;
  return playing.duration_seconds === null ? reached : Math.min(reached, playing.duration_seconds);
}

/** The time, once a second, for as long as `ticking`. */
export function useEverySecond(ticking: boolean): number {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    if (!ticking) {
      return;
    }
    setNow(Date.now());
    const beat = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(beat);
  }, [ticking]);
  return now;
}
