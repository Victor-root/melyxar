import { describe, expect, it } from "vitest";
import { codecsOf, descriptionEnd } from "./fmp4";

/** A box of `type` holding `inside`. */
function box(type: string, ...inside: Uint8Array[]): Uint8Array {
  const length = 8 + inside.reduce((sum, part) => sum + part.length, 0);
  const out = new Uint8Array(length);
  new DataView(out.buffer).setUint32(0, length);
  out.set(
    [...type].map((letter) => letter.charCodeAt(0)),
    4,
  );
  let at = 8;
  for (const part of inside) {
    out.set(part, at);
    at += part.length;
  }
  return out;
}

function joined(...parts: Uint8Array[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((sum, part) => sum + part.length, 0));
  let at = 0;
  for (const part of parts) {
    out.set(part, at);
    at += part.length;
  }
  return out;
}

/* High profile, level 5.1: what the cameras of a house commonly send. */
const AVC_C = box("avcC", new Uint8Array([1, 0x64, 0x00, 0x33, 0xff]));
const WITH_SOUND = box(
  "moov",
  box("trak", box("avc1", AVC_C)),
  box("trak", box("mp4a")),
);
const PICTURE_ONLY = box("moov", box("trak", box("avc1", AVC_C)));

describe("descriptionEnd", () => {
  it("ends after the moov box, the first piece left for later", () => {
    const start = joined(box("ftyp", new Uint8Array(4)), WITH_SOUND);
    const stream = joined(start, box("moof"), box("mdat", new Uint8Array(10)));
    expect(descriptionEnd(stream)).toBe(start.length);
  });

  it("waits while the moov box has not all come", () => {
    const start = joined(box("ftyp", new Uint8Array(4)), WITH_SOUND);
    expect(descriptionEnd(start.subarray(0, start.length - 3))).toBeNull();
    expect(descriptionEnd(start.subarray(0, 10))).toBeNull();
  });
});

describe("codecsOf", () => {
  it("names the picture's profile and level, and the sound", () => {
    expect(codecsOf(WITH_SOUND)).toEqual(["avc1.640033", "mp4a.40.2"]);
  });

  it("leaves the sound out of a camera that has none", () => {
    expect(codecsOf(PICTURE_ONLY)).toEqual(["avc1.640033"]);
  });

  it("names nothing for a picture that is not H.264", () => {
    expect(
      codecsOf(box("moov", box("trak", box("hvc1", box("hvcC"))))),
    ).toEqual([]);
  });
});
