import { describe, expect, it } from "vitest";
import { ADAPTIVE_RUNGS } from "./adaptive";
import { qualityName, rungShortName } from "./quality";

const rung = (bitrate: number | null) => ADAPTIVE_RUNGS.find((one) => one.bitrate === bitrate)!;

describe("what a rung is called", () => {
  it("names a rung by its size and rate, or the rate alone when it has no size", () => {
    expect(qualityName(rung(1_500_000), "Original")).toBe("480p · 1.5 Mb/s");
    expect(qualityName(rung(40_000_000), "Original")).toBe("40 Mb/s");
    expect(qualityName(rung(null), "Original")).toBe("Original");
  });

  it("says a rung of the automatic quality in a word or two", () => {
    expect(rungShortName(rung(2_000_000), "Origine")).toBe("720p");
    expect(rungShortName(rung(40_000_000), "Origine")).toBe("40 Mb/s");
    expect(rungShortName(rung(null), "Origine")).toBe("Origine");
  });
});
