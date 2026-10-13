/*
 * Which painting an account without a picture wears.
 *
 * Settled by its name alone, so the same account wears the same painting on
 * every screen and every device, and nothing has to be stored for it. Two
 * things are picked: the drawing, and how far round the colour wheel from the
 * accent it is painted, so a household reads as several different faces while
 * every one of them still follows the accent somebody chose.
 */

export const FACE_DRAWINGS = [
  "planet",
  "eclipse",
  "rings",
  "silk",
  "horizon",
  "record",
  "nebula",
  "orbits",
] as const;

export type FaceDrawing = (typeof FACE_DRAWINGS)[number];

/** Degrees round the colour wheel from the accent. The first is the accent
 *  itself; the rest are chosen to sit well beside it rather than evenly. */
export const FACE_TURNS = [0, 170, 205, 40, 285, 325] as const;

export interface FaceArt {
  drawing: FaceDrawing;
  turn: number;
}

/** FNV-1a over the name's code points: small, fast and well spread. */
function hashOf(name: string): number {
  let hash = 0x811c9dc5;
  for (const character of name) {
    hash ^= character.codePointAt(0) ?? 0;
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return hash;
}

export function faceArtOf(name: string): FaceArt {
  const hash = hashOf(name);
  return {
    drawing: FACE_DRAWINGS[hash % FACE_DRAWINGS.length],
    turn: FACE_TURNS[Math.floor(hash / FACE_DRAWINGS.length) % FACE_TURNS.length],
  };
}
