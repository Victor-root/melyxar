/*
 * The pictures an account without a photo of its own wears.
 *
 * Which one is the server's to say: it hands each account its own when the
 * account is made, and sends it with the account everywhere.
 */

import aurora from "./face-pictures/aurora.webp";
import binary from "./face-pictures/binary.webp";
import comet from "./face-pictures/comet.webp";
import crescent from "./face-pictures/crescent.webp";
import crystal from "./face-pictures/crystal.webp";
import eclipse from "./face-pictures/eclipse.webp";
import orbits from "./face-pictures/orbits.webp";
import rings from "./face-pictures/rings.webp";
import spiral from "./face-pictures/spiral.webp";
import verdant from "./face-pictures/verdant.webp";

/** The pictures, by the names the server gives them. */
export const FACE_PICTURES = ["comet", "rings", "orbits", "crystal", "eclipse", "aurora", "crescent", "spiral", "binary", "verdant"] as const;

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
  spiral,
  binary,
  verdant,
};
