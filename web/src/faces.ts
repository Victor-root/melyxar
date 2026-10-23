/*
 * Which picture an account without one of its own wears.
 *
 * Settled by its name alone, so the same account wears the same picture on
 * every screen and every device, the television included, and nothing has to
 * be stored for it.
 */

import aurora from "./face-pictures/aurora.webp";
import comet from "./face-pictures/comet.webp";
import crescent from "./face-pictures/crescent.webp";
import crystal from "./face-pictures/crystal.webp";
import eclipse from "./face-pictures/eclipse.webp";
import orbits from "./face-pictures/orbits.webp";
import rings from "./face-pictures/rings.webp";

/** The pictures, in the order the name's hash picks from, which the
 *  television's list follows too. */
export const FACE_PICTURES = ["comet", "rings", "orbits", "crystal", "eclipse", "aurora", "crescent"] as const;

export type FacePicture = (typeof FACE_PICTURES)[number];

/** Where each one is served from, a name the build gives it so a browser may
 *  keep it for good. */
export const FACE_ADDRESSES: Record<FacePicture, string> = {
  comet,
  rings,
  orbits,
  crystal,
  eclipse,
  aurora,
  crescent,
};

/** FNV-1a over the name's code points: small, fast and well spread. */
function hashOf(name: string): number {
  let hash = 0x811c9dc5;
  for (const character of name) {
    hash ^= character.codePointAt(0) ?? 0;
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return hash;
}

export function facePictureOf(name: string): FacePicture {
  return FACE_PICTURES[hashOf(name) % FACE_PICTURES.length];
}
