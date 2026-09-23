/*
 * The face of an account: the picture it chose, or its initials where it
 * chose none. One drawing for every place an account is shown, so a picture
 * chosen in the settings is the one the header and the door wear at once.
 */

/** Up to two letters, from the first two words of a name. */
function initialsOf(name: string): string {
  const parts = name.trim().split(/\s+/).filter(Boolean);
  if (parts.length === 0) {
    return "?";
  }
  const letters = parts.length > 1 ? `${parts[0][0]}${parts[1][0]}` : parts[0].slice(0, 1);
  return letters.toLocaleUpperCase();
}

export function Face({
  name,
  avatar,
  className,
  letters = initialsOf(name),
}: {
  name: string;
  avatar: string | null;
  /** The shape and size, which belong to where it is drawn. */
  className: string;
  /** What stands in for a picture, where two letters are not what is wanted. */
  letters?: string;
}) {
  return (
    <span className={`${className}${avatar ? " face-picture" : ""}`} aria-hidden="true">
      {avatar ? <img src={avatar} alt="" draggable={false} /> : letters}
    </span>
  );
}
