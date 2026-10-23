/*
 * The face of an account: the picture it chose, or one of Melyxar's own
 * where it chose none. One drawing for every place an account is shown, so a
 * picture chosen in the settings is the one the header and the door wear at
 * once, and an account without one wears the same picture everywhere too.
 */

import { FACE_ADDRESSES, facePictureOf } from "../faces";

export function Face({
  name,
  avatar,
  className,
}: {
  name: string;
  avatar: string | null;
  /** The shape and size, which belong to where it is drawn. */
  className: string;
}) {
  return (
    <span className={`${className} face-picture`} aria-hidden="true">
      <img src={avatar ?? FACE_ADDRESSES[facePictureOf(name)]} alt="" draggable={false} />
    </span>
  );
}
