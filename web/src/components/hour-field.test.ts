import { describe, expect, it } from "vitest";
import { HOURS, minutesOffered } from "./hour-field";

describe("an hour chosen in lists", () => {
  it("offers every hour of the day and every five minutes", () => {
    expect(HOURS).toHaveLength(24);
    expect(HOURS[0]).toBe("00");
    expect(minutesOffered("")).toEqual(["00", "05", "10", "15", "20", "25", "30", "35", "40", "45", "50", "55"]);
  });

  it("keeps a minute set elsewhere in its place among them", () => {
    expect(minutesOffered("17")).toContain("17");
    expect(minutesOffered("17").indexOf("17")).toBe(minutesOffered("17").indexOf("20") - 1);
    expect(minutesOffered("15")).toHaveLength(12);
  });
});
