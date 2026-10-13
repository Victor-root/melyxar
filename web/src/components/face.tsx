/*
 * The face of an account: the picture it chose, or a painting of its own
 * where it chose none. One drawing for every place an account is shown, so a
 * picture chosen in the settings is the one the header and the door wear at
 * once, and an account without one wears the same painting everywhere too.
 */

import { faceArtOf } from "../faces";
import { FaceArt } from "./face-art";

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
  if (avatar) {
    return (
      <span className={`${className} face-picture`} aria-hidden="true">
        <img src={avatar} alt="" draggable={false} />
      </span>
    );
  }
  const art = faceArtOf(name);
  return (
    <span
      className={`${className} face-picture face-art`}
      style={{ "--art-turn": art.turn } as React.CSSProperties}
      aria-hidden="true"
    >
      <FaceArt drawing={art.drawing} />
    </span>
  );
}
