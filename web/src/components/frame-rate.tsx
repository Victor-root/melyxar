/*
 * How many frames this device draws each second, and how long the slowest
 * one took, in a corner of the screen: switched on in the appearance
 * settings, on this device alone, for seeing where the interface stutters.
 *
 * Counted by asking the browser for every frame, which is what an animation
 * gets: a frame that comes late here came late for everything on the page.
 */

import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { deviceSwitch } from "../device-switch";
import { useSettings } from "../settings";
import { useFillingTheScreen } from "./filling-the-screen";

/** How often the corner says what it counted. */
const SAID_EVERY_MS = 1000;

/** Whether the counter is shown on this device, and the switch for it. */
export const useFrameRateShown = deviceSwitch("melyxar.frame_rate").useSwitch;

export function FrameRateMeter() {
  const [on] = useFrameRateShown();
  return on ? <Counter /> : null;
}

function Counter() {
  const { t } = useSettings();
  const filling = useFillingTheScreen();
  const [said, setSaid] = useState<{ perSecond: number; slowestMs: number } | null>(null);

  useEffect(() => {
    let frames = 0;
    let slowest = 0;
    let last = performance.now();
    let since = last;
    let asked = 0;
    const frame = (now: number) => {
      frames += 1;
      slowest = Math.max(slowest, now - last);
      last = now;
      if (now - since >= SAID_EVERY_MS) {
        setSaid({ perSecond: Math.round((frames * 1000) / (now - since)), slowestMs: Math.round(slowest) });
        frames = 0;
        slowest = 0;
        since = now;
      }
      asked = requestAnimationFrame(frame);
    };
    asked = requestAnimationFrame(frame);
    return () => cancelAnimationFrame(asked);
  }, []);

  if (!said) {
    return null;
  }
  return createPortal(
    <div className="frame-rate" aria-hidden="true">
      {t("frame_rate.meter", { fps: said.perSecond, ms: said.slowestMs })}
    </div>,
    filling ?? document.body,
  );
}
