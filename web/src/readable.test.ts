/*
 * The sums behind the figures the interface writes out.
 *
 * Every one of these turns a number the server sent into something read on
 * the screen, and a wrong one reads exactly like a right one: ninety nine per
 * cent and a hundred are both plausible, two hours forty one and two hours
 * fourteen are both plausible, and nobody checks a file size against a
 * calculator. That is what these are for.
 */

import { describe, expect, it } from "vitest";
import type { Wording } from "./readable";
import {
  amountOfData,
  asLocalTime,
  asUtcMinutes,
  containerName,
  howLong,
  howLongSince,
  insideTheRange,
  networkRate,
  outOfAHundred,
  outOfTen,
  percentOf,
  readableBitrate,
  readableSize,
  releaseOf,
} from "./readable";

/** Standing in for the interface's own wording, so what is checked is the sum
 *  and not the sentence it ends up in. */
const said: Wording = (key, values) =>
  values ? `${key}(${JSON.stringify(values)})` : key;

describe("outOfAHundred", () => {
  it("rounds down, so a hundred is only ever a real hundred", () => {
    expect(outOfAHundred(0)).toBe(0);
    expect(outOfAHundred(0.5)).toBe(50);
    expect(outOfAHundred(0.999)).toBe(99);
    /* The one that was seen on a scan: the last film of three hundred and
       forty seven left to read, and the bar saying it was done. */
    expect(outOfAHundred(346 / 347)).toBe(99);
  });

  it("says a hundred once it really is finished", () => {
    expect(outOfAHundred(1)).toBe(100);
    expect(outOfAHundred(1.2)).toBe(100);
  });
});

describe("howLong", () => {
  it("says minutes on their own under the hour", () => {
    expect(howLong(0, said)).toBe('work.minutes({"count":0})');
    expect(howLong(59, said)).toBe('work.minutes({"count":59})');
  });

  it("says whole hours without a nought of minutes after them", () => {
    expect(howLong(60, said)).toBe('work.hours({"hours":1})');
    expect(howLong(120, said)).toBe('work.hours({"hours":2})');
  });

  it("splits the rest off the hours", () => {
    expect(howLong(155, said)).toBe('work.hours_minutes({"hours":2,"minutes":35})');
    expect(howLong(61, said)).toBe('work.hours_minutes({"hours":1,"minutes":1})');
  });
});

describe("the nightly hour, there and back", () => {
  it("comes back to the minute it started at", () => {
    for (const minutes of [0, 1, 185, 720, 1439]) {
      expect(asUtcMinutes(asLocalTime(minutes))).toBe(minutes);
    }
  });

  it("writes two figures either side, whatever the hour", () => {
    expect(asLocalTime(asUtcMinutes("03:05"))).toBe("03:05");
    expect(asLocalTime(asUtcMinutes("00:00"))).toBe("00:00");
    expect(asLocalTime(asUtcMinutes("23:59"))).toBe("23:59");
  });

  it("reads a field somebody emptied as midnight rather than as nothing", () => {
    expect(asUtcMinutes("")).toBe(0);
    expect(asUtcMinutes("half past two")).toBe(0);
  });
});

describe("readableSize", () => {
  it("climbs a unit at a thousand, not at a thousand and twenty four", () => {
    expect(readableSize(0)).toBe("0 B");
    expect(readableSize(999)).toBe("999 B");
    expect(readableSize(1000)).toBe("1.0 kB");
    expect(readableSize(1_500_000_000)).toBe("1.5 GB");
  });

  it("stops at the largest unit it knows rather than running off the end", () => {
    expect(readableSize(5_000_000_000_000)).toBe("5.0 TB");
    expect(readableSize(9_000_000_000_000_000)).toBe("9000.0 TB");
  });
});

describe("readableBitrate", () => {
  it("says nothing at all where the file said nothing", () => {
    expect(readableBitrate(null)).toBeNull();
    expect(readableBitrate(0)).toBeNull();
    expect(readableBitrate(-1)).toBeNull();
  });

  it("changes unit at the million", () => {
    expect(readableBitrate(999_999)).toBe("1000 kb/s");
    expect(readableBitrate(1_000_000)).toBe("1.0 Mb/s");
    expect(readableBitrate(8_500_000)).toBe("8.5 Mb/s");
  });
});

describe("outOfTen", () => {
  it("keeps one figure after the point, on a round mark as on any other", () => {
    expect(outOfTen(7)).toBe("7.0 / 10");
    /* Either side of the half rather than on it: a mark of exactly five
       thousandths is not a number a machine holds exactly, so which way it
       goes is the machine's business and not something to be written down as
       though it were a rule. */
    expect(outOfTen(7.84)).toBe("7.8 / 10");
    expect(outOfTen(7.86)).toBe("7.9 / 10");
  });
});

describe("insideTheRange", () => {
  it("stops at either bound", () => {
    expect(insideTheRange(5, 10, 20)).toBe(10);
    expect(insideTheRange(50, 10, 20)).toBe(20);
    expect(insideTheRange(15, 10, 20)).toBe(15);
  });

  it("rounds what a field hands back with a point in it", () => {
    expect(insideTheRange(14.6, 10, 20)).toBe(15);
  });

  it("answers nothing for a field that holds no number", () => {
    expect(insideTheRange(Number.NaN, 10, 20)).toBeNull();
  });
});

describe("howLongSince", () => {
  const start = "2026-09-01T10:00:00Z";
  const later = (minutes: number) => new Date(start).getTime() + minutes * 60_000;

  it("says days and hours once a day has gone, and drops the minutes", () => {
    expect(howLongSince(start, later(12 * 1440 + 4 * 60 + 37), said)).toBe(
      'time.days_hours({"days":12,"hours":4})',
    );
  });

  it("says hours and minutes within a day", () => {
    expect(howLongSince(start, later(3 * 60 + 5), said)).toBe(
      'time.hours_minutes({"hours":3,"minutes":5})',
    );
  });

  it("says minutes alone within an hour, and never less than none", () => {
    expect(howLongSince(start, later(8), said)).toBe('time.minutes({"minutes":8})');
    expect(howLongSince(start, later(-3), said)).toBe('time.minutes({"minutes":0})');
  });
});

describe("releaseOf", () => {
  it("keeps the release and drops the commit it was built from", () => {
    expect(releaseOf("0.1.0 (28a4bd86+edited)")).toBe("0.1.0");
    expect(releaseOf("0.2.0")).toBe("0.2.0");
  });
});

describe("amountOfData", () => {
  it("writes the units and the decimal mark of the language spoken", () => {
    expect(amountOfData(6.2 * 1024 ** 3, "en")).toBe("6.2\u00a0GB");
    expect(amountOfData(6.2 * 1024 ** 3, "fr")).toBe("6,2\u00a0Go");
    expect(amountOfData(2.4 * 1024 ** 4, "fr")).toBe("2,4\u00a0To");
  });

  it("counts by 1024, so a container given 8192 MB holds 8 GB", () => {
    expect(amountOfData(8192 * 1024 ** 2, "fr")).toBe("8\u00a0Go");
  });

  it("keeps no decimal where it would only be noise", () => {
    expect(amountOfData(512, "en")).toBe("512\u00a0B");
    expect(amountOfData(125.4 * 1024 ** 2, "en")).toBe("125\u00a0MB");
  });
});

describe("networkRate", () => {
  it("counts in bits a second, as a connection is sold", () => {
    expect(networkRate(15_625_000, "en")).toBe("125\u00a0Mb/s");
    expect(networkRate(1_500_000, "fr")).toBe("12,0\u00a0Mb/s");
    expect(networkRate(0, "en")).toBe("0\u00a0b/s");
  });
});

describe("percentOf", () => {
  it("rounds a share to a whole percentage", () => {
    expect(percentOf(0.184, "en")).toBe("18%");
    expect(percentOf(0.184, "fr")).toBe("18\u00a0%");
  });
});

describe("containerName", () => {
  it("calls a container by the extension its files carry", () => {
    expect(containerName("matroska,webm")).toBe("MKV");
    expect(containerName("mov,mp4,m4a,3gp,3g2,mj2")).toBe("MP4");
    expect(containerName("mpegts")).toBe("MPEG-TS");
  });

  it("keeps the analyser's first name for one it has no other name for", () => {
    expect(containerName("avi")).toBe("AVI");
    expect(containerName("flv")).toBe("FLV");
  });
});
