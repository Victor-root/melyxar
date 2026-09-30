/*
 * What the page of security works out about the way in: where the browser
 * goes once it changes, and whether a certificate is close to its end.
 */

import type { AccessMode } from "../../api";

/** How many days before its end a certificate is said to be running out. */
const RUNNING_OUT_DAYS = 30;

/**
 * Where this page goes once the way in has changed, or nothing when it can
 * stay: an encrypted server sends a page in the clear to https, and behind a
 * proxy the server stops answering https on its own port.
 */
export function addressAfter(mode: AccessMode, encrypted: boolean, location: Location | URL): string | null {
  const wanted = mode === "proxy" ? "http:" : "https:";
  if (location.protocol === wanted || (mode === "proxy" && !encrypted)) {
    return null;
  }
  return `${wanted}//${location.host}${location.pathname}${location.search}`;
}

/** Whether a certificate ends within a month, or has already. */
export function runningOut(notAfter: string, now: Date): boolean {
  return new Date(notAfter).getTime() - now.getTime() < RUNNING_OUT_DAYS * 24 * 3600 * 1000;
}
