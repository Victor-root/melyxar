/*
 * The face of an account: the picture it chose, or the one of Melyxar's own
 * the server gave it. One drawing for every place an account is shown, so a
 * picture chosen in the settings is the one the header and the door wear at
 * once, and an account without one wears the same picture everywhere too.
 */

import { FACE_ADDRESSES, FACE_PICTURES, type FacePicture } from "../faces";

export function Face({
  avatar,
  picture,
  className,
}: {
  avatar: string | null;
  /** Melyxar's own picture it wears without a photo; the first while the
   *  account is not known yet. */
  picture: FacePicture | null;
  /** The shape and size, which belong to where it is drawn. */
  className: string;
}) {
  return (
    <span className={`${className} face-picture`} aria-hidden="true">
      <img src={avatar ?? FACE_ADDRESSES[picture ?? FACE_PICTURES[0]]} alt="" draggable={false} />
    </span>
  );
}
