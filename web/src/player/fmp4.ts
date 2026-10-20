/*
 * What the start of a fragmented MP4 says, read before a browser is handed
 * the rest: where its description ends, and what it is made of, which the
 * browser has to be told before it takes a single piece.
 */

/** The four letters of a box's type, at `at`. */
function typeAt(bytes: Uint8Array, at: number): string {
  return String.fromCharCode(
    bytes[at],
    bytes[at + 1],
    bytes[at + 2],
    bytes[at + 3],
  );
}

function sizeAt(bytes: Uint8Array, at: number): number {
  return (
    ((bytes[at] << 24) |
      (bytes[at + 1] << 16) |
      (bytes[at + 2] << 8) |
      bytes[at + 3]) >>>
    0
  );
}

/** Where the description of the stream ends, its `moov` box included, or
 *  null while it has not all come yet. */
export function descriptionEnd(bytes: Uint8Array): number | null {
  let at = 0;
  while (at + 8 <= bytes.length) {
    const size = sizeAt(bytes, at);
    if (size < 8) {
      return null;
    }
    const end = at + size;
    if (typeAt(bytes, at + 4) === "moov") {
      return end <= bytes.length ? end : null;
    }
    at = end;
  }
  return null;
}

/** Where the four letters `type` first appear, or -1. */
function find(bytes: Uint8Array, type: string): number {
  for (let at = 0; at + 4 <= bytes.length; at += 1) {
    if (typeAt(bytes, at) === type) {
      return at;
    }
  }
  return -1;
}

function hex(byte: number): string {
  return byte.toString(16).toUpperCase().padStart(2, "0");
}

/**
 * What the stream is made of, as a browser is told it: the picture's H.264
 * profile and level as its own description gives them, and AAC for the sound
 * when there is one, which is what the server makes of any camera's sound.
 * Empty when the picture is in a form this cannot name.
 */
export function codecsOf(description: Uint8Array): string[] {
  const avc = find(description, "avcC");
  if (avc < 0 || avc + 8 > description.length) {
    return [];
  }
  /* After the version, the profile, its constraints and the level. */
  const codecs = [
    `avc1.${hex(description[avc + 5])}${hex(description[avc + 6])}${hex(description[avc + 7])}`,
  ];
  if (find(description, "mp4a") >= 0) {
    codecs.push("mp4a.40.2");
  }
  return codecs;
}
