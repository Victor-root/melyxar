import { describe, expect, it } from "vitest";
import { diskName, fullness, usedShare } from "./disks";

describe("usedShare", () => {
  it("is what is taken of the whole", () => {
    expect(usedShare({ total_bytes: 4000, available_bytes: 1000 })).toBe(0.75);
    expect(usedShare({ total_bytes: 0, available_bytes: 0 })).toBe(0);
  });
});

describe("fullness", () => {
  it("asks for an eye past four fifths and a look past nine tenths", () => {
    expect(fullness(0.79)).toBeNull();
    expect(fullness(0.8)).toBe("attention");
    expect(fullness(0.89)).toBe("attention");
    expect(fullness(0.9)).toBe("trouble");
  });
});

describe("diskName", () => {
  it("is the folder the disk is mounted on", () => {
    expect(diskName("/mnt/Big disk")).toBe("Big disk");
    expect(diskName("/mnt/Big disk/")).toBe("Big disk");
  });

  it("is nothing for the disk of the system", () => {
    expect(diskName("/")).toBeNull();
  });
});
