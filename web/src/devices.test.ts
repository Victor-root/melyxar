import { describe, expect, it } from "vitest";
import type { SignedInDevice } from "./api";
import { aboutDevice, brandOf, deviceKind, deviceName, deviceSaid, lastSeen } from "./devices";

const WINDOWS_CHROME =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36";
const WINDOWS_EDGE = `${WINDOWS_CHROME} Edg/140.0.0.0`;
const LINUX_FIREFOX = "Mozilla/5.0 (X11; Linux x86_64; rv:143.0) Gecko/20100101 Firefox/143.0";
const IPHONE_SAFARI =
  "Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1";
const ANDROID_CHROME =
  "Mozilla/5.0 (Linux; Android 15; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Mobile Safari/537.36";
const MAC_SAFARI =
  "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Safari/605.1.15";

const t = (key: string, values?: Record<string, string | number>) =>
  key === "device.on" ? `${values?.browser} on ${values?.system}` : key;

describe("deviceName", () => {
  it("names the television application by the television's model", () => {
    expect(deviceName("SHIELD Android TV", t, "Melyxar")).toBe("Melyxar on SHIELD Android TV");
    expect(deviceName("MelyxarTV/1.0 (Linux; Android 12; BRAVIA) SmartTV", t, "Melyxar")).toBe(
      "Melyxar on BRAVIA",
    );
    expect(
      deviceName("MelyxarTV/0.1.0-20261107-1630 (Linux; Android 14; GLKM-MINI2) SmartTV", t, "Melyxar"),
    ).toBe("Melyxar on GLKM-MINI2");
    expect(deviceName(ANDROID_CHROME, t)).toBe("Chrome on Android web");
  });

  it("names the telephone application as Melyxar, whatever its page found", () => {
    expect(deviceName(`${ANDROID_CHROME} MelyxarApp/0.4.0`, t, "Android WebView")).toBe("Melyxar on Android");
    expect(deviceKind(`${ANDROID_CHROME} MelyxarApp/0.4.0`, "Android WebView")).toBe("phone");
  });
});

describe("deviceKind", () => {
  it("tells a television, a telephone and a computer apart", () => {
    expect(deviceKind("SHIELD Android TV", "Melyxar")).toBe("tv");
    expect(deviceKind("MelyxarTV/1.0 (Linux; Android 12; BRAVIA) SmartTV")).toBe("tv");
    expect(deviceKind(ANDROID_CHROME)).toBe("phone");
    expect(deviceKind(IPHONE_SAFARI)).toBe("phone");
    expect(deviceKind(WINDOWS_CHROME, "Brave")).toBe("computer");
  });
});

describe("deviceSaid", () => {
  it("names the browser built on top rather than the one underneath", () => {
    expect(deviceSaid(WINDOWS_CHROME)).toEqual({ browser: "Chrome", system: "Windows" });
    expect(deviceSaid(WINDOWS_EDGE)).toEqual({ browser: "Edge", system: "Windows" });
  });

  it("tells a phone from the system its own is built on", () => {
    expect(deviceSaid(ANDROID_CHROME)).toEqual({ browser: "Chrome", system: "Android" });
    expect(deviceSaid(IPHONE_SAFARI)).toEqual({ browser: "Safari", system: "iOS" });
  });

  it("reads the rest", () => {
    expect(deviceSaid(LINUX_FIREFOX)).toEqual({ browser: "Firefox", system: "Linux" });
    expect(deviceSaid(MAC_SAFARI)).toEqual({ browser: "Safari", system: "macOS" });
  });
});

describe("deviceName", () => {
  it("says both when both are known, one when only one is", () => {
    expect(deviceName(WINDOWS_CHROME, t)).toBe("Chrome on Windows");
    expect(deviceName("SomeTool/1.0 (Linux)", t)).toBe("Linux");
  });

  it("says it does not know rather than showing the line", () => {
    expect(deviceName("an unnamed device", t)).toBe("device.unknown");
  });

  it("takes the browser its page found over the one its line names", () => {
    expect(deviceName(WINDOWS_CHROME, t, "Brave")).toBe("Brave on Windows");
  });
});

describe("brandOf", () => {
  it("finds the browser behind its engine and the brand every one of them makes up", () => {
    expect(
      brandOf([{ brand: "Brave" }, { brand: "Chromium" }, { brand: "Not=A?Brand" }]),
    ).toBe("Brave");
    expect(
      brandOf([{ brand: "Not;A=Brand" }, { brand: "Google Chrome" }, { brand: "Chromium" }]),
    ).toBe("Chrome");
    expect(brandOf([{ brand: "Microsoft Edge" }, { brand: "Chromium" }])).toBe("Edge");
  });

  it("finds nothing where there is only the engine", () => {
    expect(brandOf([{ brand: "Chromium" }, { brand: "Not_A Brand" }])).toBeNull();
    expect(brandOf([])).toBeNull();
  });
});

/** Standing in for the wording, so what is checked is what goes into it. */
const worded = (key: string, values?: Record<string, string | number>) =>
  values ? `${key}(${Object.values(values).join("|")})` : key;

const NOW = Date.parse("2026-03-10T12:00:00Z");

describe("lastSeen", () => {
  it("says just now within the minute, and how long ago after it", () => {
    expect(lastSeen("2026-03-10T11:59:30Z", NOW, worded)).toBe("device.active_now");
    expect(lastSeen("2026-03-10T11:55:00Z", NOW, worded)).toBe("device.last_seen(time.minutes(5))");
  });
});

describe("aboutDevice", () => {
  const device: SignedInDevice = {
    id: "d",
    user_id: "u",
    user_name: "somebody",
    user_avatar: null,
    user_face_picture: "comet",
    name: WINDOWS_CHROME,
    browser: null,
    remembered: false,
    signed_in_at: "2026-03-01T09:00:00Z",
    last_seen_at: "2026-03-10T10:00:00Z",
    is_this_one: false,
  };

  it("says whose it is only when asked, and that it ends with the browser when it does", () => {
    expect(aboutDevice(device, true, NOW, "en", worded)).toMatch(
      /^somebody · device\.signed_in_on\(.*2026.*\) · device\.last_seen\(time\.hours_minutes\(2\|0\)\) · device\.until_closed$/,
    );
    expect(aboutDevice({ ...device, remembered: true }, false, NOW, "en", worded)).toMatch(
      /^device\.signed_in_on\(.*\) · device\.last_seen\(time\.hours_minutes\(2\|0\)\)$/,
    );
  });

  it("says the device looked from is in use now, whatever was last written down", () => {
    expect(aboutDevice({ ...device, is_this_one: true }, false, NOW, "en", worded)).toMatch(
      / · device\.active_now · device\.until_closed$/,
    );
  });
});
