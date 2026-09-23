import { describe, expect, it } from "vitest";
import { deviceName, deviceSaid } from "./devices";

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
});
