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
export function deviceName(
  line: string,
  t: (key: string, values?: Record<string, string | number>) => string,
  found: string | null = null,
): string {
  const said = deviceSaid(line);
  const browser = found ?? said.browser;
  const system = said.system;
  if (browser && system) {
    return t("device.on", { browser, system });
  }
  return browser ?? system ?? t("device.unknown");
}
