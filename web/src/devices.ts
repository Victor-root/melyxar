/*
 * What a device is, read from what its browser said about itself when it
 * signed in, and from what its own page found.
 *
 * That line is written for machines and reads like one. What somebody looks
 * for in a list is "Firefox on Windows", so that is what is drawn, and the
 * line itself stays in the details for whoever wants the whole of it. Some
 * browsers send the line of another on purpose, Brave sending Chrome's, and
 * only the page can ask them what they really are.
 */

import type { SignedInDevice } from "./api";
import { howLongSince, readableDate } from "./readable";
import type { Wording } from "./readable";

/** The browser and the system it runs on, each when they could be told. */
export interface DeviceSaid {
  browser: string | null;
  system: string | null;
}

/* Looked for in order: a browser built on another names that one too, so
   the one built on top is looked for first. */
const BROWSERS: [RegExp, string][] = [
  [/\bEdgA?\//, "Edge"],
  [/\b(OPR|Opera)\//, "Opera"],
  [/\bSamsungBrowser\//, "Samsung Internet"],
  [/\bVivaldi\//, "Vivaldi"],
  [/\b(Firefox|FxiOS)\//, "Firefox"],
  [/\b(Chrome|Chromium|CriOS)\//, "Chrome"],
  [/\bVersion\/[\d.]+.*\bSafari\//, "Safari"],
];

const SYSTEMS: [RegExp, string][] = [
  [/\b(Tizen|Web0S|webOS|SmartTV|SMART-TV)\b/, "TV"],
  [/\bAndroid\b/, "Android"],
  [/\b(iPhone|iPad|iPod)\b/, "iOS"],
  [/\bCrOS\b/, "ChromeOS"],
  [/\bWindows\b/, "Windows"],
  [/\bMac OS X\b|\bMacintosh\b/, "macOS"],
  [/\bLinux\b/, "Linux"],
];

function firstOf(said: string, known: [RegExp, string][]): string | null {
  return known.find(([pattern]) => pattern.test(said))?.[1] ?? null;
}

/** What a browser's line about itself says, as far as it can be read. */
export function deviceSaid(line: string): DeviceSaid {
  return { browser: firstOf(line, BROWSERS), system: firstOf(line, SYSTEMS) };
}

/* What a browser calls itself among the brands it gives, as a list names it.
   Its engine and the made up brand every one of them adds say nothing. */
const BRAND_NAMES: Record<string, string> = {
  "Google Chrome": "Chrome",
  "Microsoft Edge": "Edge",
};

/** The browser among the brands a browser gives, when one is more than its
 *  engine. */
export function brandOf(brands: { brand: string }[]): string | null {
  const own = brands
    .map(({ brand }) => brand.trim())
    .find((brand) => brand !== "Chromium" && !/not.?a.?brand/i.test(brand));
  return own ? (BRAND_NAMES[own] ?? own) : null;
}

interface WhatABrowserAnswers {
  brave?: { isBrave?: () => Promise<boolean> };
  userAgentData?: { brands?: { brand: string }[] };
}

/**
 * The browser this page runs in, asked of the browser itself, when it can
 * tell more than the line it sends. Brave answers whoever asks; the brands
 * are given only over a secured connection.
 */
export async function foundBrowser(): Promise<string | null> {
  const asked = navigator as Navigator & WhatABrowserAnswers;
  if (await asked.brave?.isBrave?.().catch(() => false)) {
    return "Brave";
  }
  return brandOf(asked.userAgentData?.brands ?? []);
}

/** The name a list shows for a device: the browser its page found when it
 *  found one, otherwise the one its line names. */
export function deviceName(line: string, t: Wording, found: string | null = null): string {
  const said = deviceSaid(line);
  const browser = found ?? said.browser;
  const system = said.system;
  if (browser && system) {
    return t("device.on", { browser, system });
  }
  return browser ?? system ?? t("device.unknown");
}

const A_MINUTE_MS = 60_000;

/** When a device or an account was last about: just now, or how long ago. */
export function lastSeen(instant: string, now: number, t: Wording): string {
  return now - new Date(instant).getTime() < A_MINUTE_MS
    ? t("device.active_now")
    : t("device.last_seen", { when: howLongSince(instant, now, t) });
}

/**
 * What a list says under a device: whose it is when the list holds more than
 * one account's, when it signed in, when it was last used, and whether its
 * session ends with the browser.
 *
 * The device the list is looked at from is in use this very moment, whatever
 * the server last wrote down: it writes the last use at most once an hour.
 */
export function aboutDevice(
  device: SignedInDevice,
  withAccount: boolean,
  now: number,
  language: string,
  t: Wording,
): string {
  return [
    withAccount ? device.user_name : null,
    t("device.signed_in_on", {
      date: readableDate(device.signed_in_at, language) ?? device.signed_in_at,
    }),
    device.is_this_one ? t("device.active_now") : lastSeen(device.last_seen_at, now, t),
    device.remembered ? null : t("device.until_closed"),
  ]
    .filter((part): part is string => part !== null)
    .join(" · ");
}
