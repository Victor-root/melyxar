/*
 * Where maintenance stands, held once for the page.
 *
 * Asked as the page opens, told on the live line whenever it moves, and
 * learnt from any request the server turns away for it. At the door, and
 * while this page is turned away, there is no live line to tell, so it is
 * asked again every little while instead.
 *
 * The countdown is counted here from the seconds the server gave, never from
 * an hour: a device whose clock is wrong still flips at the right moment.
 */

import { useEffect, useSyncExternalStore } from "react";
import { api, whenMaintenanceTurnsAway } from "./api";
import type { Maintenance, MaintenanceState } from "./api";

/** Maintenance as this page holds it. */
export interface Standing {
  state: MaintenanceState;
  /** When it takes hold, on this device's clock, while announced. */
  startsAt: number | null;
  until: string | null;
  message: string | null;
}

/** How often it is asked again when nothing else would tell. */
const ASKED_AGAIN_EVERY_MS = 10_000;

const OFF: Standing = { state: "off", startsAt: null, until: null, message: null };

/** What the server said, as held from the moment it was heard. */
export function standingOf(seen: Maintenance, heardAt: number): Standing {
  return {
    state: seen.state,
    startsAt:
      seen.state === "announced" && seen.starts_in_seconds !== null
        ? heardAt + seen.starts_in_seconds * 1000
        : null,
    until: seen.until,
    message: seen.message,
  };
}

/**
 * The next time this hour of the day comes, on this device's clock, after
 * the given instant: today if it is still to come, tomorrow otherwise. What
 * an hour typed for the end of a maintenance means.
 */
export function nextAt(hourOfDay: string, after: number): Date {
  const [hours, minutes] = hourOfDay.split(":").map(Number);
  const at = new Date(after);
  at.setHours(hours, minutes, 0, 0);
  if (at.getTime() <= after) {
    at.setDate(at.getDate() + 1);
  }
  return at;
}

/** An instant as the hour of the day a time field holds, on this device. */
export function hourOfDayOf(instant: string): string {
  const at = new Date(instant);
  return `${String(at.getHours()).padStart(2, "0")}:${String(at.getMinutes()).padStart(2, "0")}`;
}

/** Where it stands at this instant: an announcement whose time has come is in
 *  force, whether or not the server has said so yet. */
export function stateAt(standing: Standing, now: number): MaintenanceState {
  return standing.state === "announced" && standing.startsAt !== null && now >= standing.startsAt
    ? "on"
    : standing.state;
}

let held: Standing = OFF;
let flip: number | undefined;
const listeners = new Set<() => void>();

function tell() {
  for (const listener of listeners) listener();
}

/** Takes what the server said, from any of the three ways it says it. */
export function maintenanceHeard(seen: Maintenance): void {
  held = standingOf(seen, Date.now());
  window.clearTimeout(flip);
  if (held.startsAt !== null) {
    flip = window.setTimeout(takesHold, Math.max(0, held.startsAt - Date.now()));
  }
  tell();
}

/** The announcement's time has come: in force here at once, the server's
 *  word following on the live line. */
function takesHold() {
  if (stateAt(held, Date.now()) === "on") {
    held = { ...held, state: "on", startsAt: null };
    tell();
  }
}

/** Asks the server where it stands. Unanswered, what was held stays. */
export function askAboutMaintenance(): void {
  api
    .maintenance()
    .then(maintenanceHeard)
    .catch(() => {
      // A server that cannot be reached says nothing about maintenance.
    });
}

function follow(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** Maintenance as it stands, drawn again whenever it moves. */
export function useMaintenance(): Standing {
  return useSyncExternalStore(follow, () => held);
}

/**
 * Follows maintenance for the whole page: asked as it opens, again when it
 * comes back into view, whenever a request is turned away for it, and every
 * little while for as long as nothing else would tell: the page turned away,
 * or at the door, where no live line is held.
 */
export function useFollowedMaintenance(asking: boolean): void {
  useEffect(() => {
    askAboutMaintenance();
    whenMaintenanceTurnsAway(askAboutMaintenance);
    const lookAgain = () => {
      if (document.visibilityState === "visible") askAboutMaintenance();
    };
    document.addEventListener("visibilitychange", lookAgain);
    return () => {
      document.removeEventListener("visibilitychange", lookAgain);
      whenMaintenanceTurnsAway(() => {});
    };
  }, []);

  useEffect(() => {
    if (!asking) {
      return;
    }
    const again = window.setInterval(askAboutMaintenance, ASKED_AGAIN_EVERY_MS);
    return () => window.clearInterval(again);
  }, [asking]);
}
