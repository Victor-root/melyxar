/*
 * A yes or no kept on this device alone, in its browser's storage rather than
 * with the account: what suits one screen says nothing of the others the
 * same person uses.
 */

import { useSyncExternalStore } from "react";
import { safeRead, safeWrite } from "./i18n";

export interface DeviceSwitch {
  /** Where it is now, read outside a component. */
  isOn: () => boolean;
  /** Where it is, read where it is shown, and the switch for it. */
  useSwitch: () => [boolean, (on: boolean) => void];
}

/** The switch kept under that name, with what to do each time it is moved. */
export function deviceSwitch(kept: string, moved?: (on: boolean) => void): DeviceSwitch {
  let on = safeRead(kept) === "yes";
  const listeners = new Set<() => void>();
  const follow = (listener: () => void) => {
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  };
  const move = (next: boolean) => {
    on = next;
    safeWrite(kept, next ? "yes" : "no");
    moved?.(next);
    listeners.forEach((listener) => listener());
  };
  return {
    isOn: () => on,
    useSwitch: () => [useSyncExternalStore(follow, () => on), move],
  };
}
