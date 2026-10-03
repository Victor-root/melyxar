import { describe, expect, it } from "vitest";
import type { DeviceCalibration } from "../api";
import { judge, smoothHeight, whatToTellTheServer } from "./judge";

const clean = { made: 216, dropped: 1, advanced: 9, watched: 9, frameRate: 24 };

describe("judging one clip", () => {
  it("passes a clip that kept time, showed what it had to and dropped almost nothing", () => {
    expect(judge(2160, clean).passed).toBe(true);
  });

  it("fails a clip that threw away more than a few pictures", () => {
    expect(judge(2160, { ...clean, dropped: 10 }).passed).toBe(false);
  });

  it("fails a decoder too slow to make pictures, though it dropped none", () => {
    const slow = judge(2160, { ...clean, made: 60, dropped: 0 });
    expect(slow.passed).toBe(false);
    expect(slow.shown_share).toBeCloseTo(60 / 216);
  });

  it("fails a clip whose clock fell behind", () => {
    expect(judge(2160, { ...clean, advanced: 6 }).passed).toBe(false);
  });

  it("never reads nothing made as a flawless clip", () => {
    const nothing = judge(2160, { ...clean, made: 0, dropped: 0 });
    expect(nothing.passed).toBe(false);
    expect(nothing.dropped_share).toBe(1);
  });
});

describe("the tallest height a codec held up at", () => {
  it("is the first height that passed, tallest first", () => {
    expect(
      smoothHeight([
        { height: 2160, passed: false, dropped_share: 0.2, shown_share: 1 },
        { height: 1440, passed: true, dropped_share: 0, shown_share: 1 },
      ]),
    ).toBe(1440);
  });

  it("is none when no height passed", () => {
    expect(smoothHeight([{ height: 720, passed: false, dropped_share: 1, shown_share: 0 }])).toBeNull();
  });
});

describe("what the server is told", () => {
  const predicted = [
    { codec: "h264", max_height: 2160, power_efficient: true },
    { codec: "vp9", max_height: 2160, power_efficient: true },
  ];
  const takes = () => true;

  it("is the browser's own prediction, untouched, without a whole calibration", () => {
    expect(whatToTellTheServer(predicted, null, takes)).toBe(predicted);
  });

  it("is only what was measured, at the height each codec held up at", () => {
    const calibration: DeviceCalibration = {
      calibration_version: 5,
      measured_at: "2026-10-03T12:00:00Z",
      codecs: [
        { codec: "h264", smooth_height: 1080, measurements: [] },
        { codec: "av1", smooth_height: null, measurements: [] },
      ],
    };
    expect(whatToTellTheServer(predicted, calibration, takes)).toEqual([
      { codec: "h264", max_height: 1080, power_efficient: true },
    ]);
  });

  it("never offers a codec the browser cannot take in pieces", () => {
    const calibration: DeviceCalibration = {
      calibration_version: 5,
      measured_at: "2026-10-03T12:00:00Z",
      codecs: [{ codec: "hevc", smooth_height: 2160, measurements: [] }],
    };
    expect(whatToTellTheServer(predicted, calibration, () => false)).toEqual([]);
  });
});
