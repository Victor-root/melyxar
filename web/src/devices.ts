/*
 * What a device is, read from what its browser said about itself when it
 * signed in.
 *
 * That line is written for machines and reads like one. What somebody looks
 * for in a list is "Firefox on Windows", so that is what is drawn, and the
 * line itself stays in the details for whoever wants the whole of it.
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

/** The name a list shows for a device. */
export function deviceName(
  line: string,
  t: (key: string, values?: Record<string, string | number>) => string,
): string {
  const { browser, system } = deviceSaid(line);
  if (browser && system) {
    return t("device.on", { browser, system });
  }
  return browser ?? system ?? t("device.unknown");
}
