/*
 * Where maintenance stands, held once for the page.
 *
 * Asked as the page opens, told on the live line whenever it moves, and
 * learnt from any request the server turns away for it. While it is in force
 * and this page is turned away, there is no live line to tell when it ends,
 * so it is asked again every little while instead.
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

/** How often it is asked again while it is in force. */
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
 * little while for as long as this page is turned away.
 */
export function useFollowedMaintenance(turnedAway: boolean): void {
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
    if (!turnedAway) {
      return;
    }
    const asking = window.setInterval(askAboutMaintenance, ASKED_AGAIN_EVERY_MS);
    return () => window.clearInterval(asking);
  }, [turnedAway]);
}
