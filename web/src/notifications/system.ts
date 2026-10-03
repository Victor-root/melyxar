/*
 * The system's own notifications, said by the browser outside the page while
 * a Melyxar tab is open in the background. Chosen on each device, since the
 * browser's permission is given to a device and not to an account; nothing
 * reaches a closed tab, by decision.
 */

import { safeRead, safeWrite } from "../i18n";

const CHOSEN = "melyxar.notifications.system";

/** Whether this browser can say anything outside the page at all. */
export function systemCanSay(): boolean {
  return typeof window !== "undefined" && "Notification" in window;
}

/** Whether this device chose them, and the browser lets them through. */
export function systemSays(): boolean {
  return systemCanSay() && safeRead(CHOSEN) === "yes" && Notification.permission === "granted";
}

/** Whether the browser refused them for good, which only its own settings
 *  can undo. */
export function systemRefused(): boolean {
  return systemCanSay() && Notification.permission === "denied";
}

/** Chooses them on this device, asking the browser first. Answers whether
 *  they are on. */
export async function chooseSystem(wanted: boolean): Promise<boolean> {
  if (!wanted || !systemCanSay()) {
    safeWrite(CHOSEN, "no");
    return false;
  }
  const permission =
    Notification.permission === "default" ? await Notification.requestPermission() : Notification.permission;
  safeWrite(CHOSEN, permission === "granted" ? "yes" : "no");
  return permission === "granted";
}

/** Says a notification outside the page; pressing it brings the page back. */
export function sayToTheSystem(title: string, detail: string | null, icon: string | null, onOpen: () => void) {
  const said = new Notification(title, { body: detail ?? undefined, icon: icon ?? undefined });
  said.onclick = () => {
    window.focus();
    onOpen();
    said.close();
  };
}
