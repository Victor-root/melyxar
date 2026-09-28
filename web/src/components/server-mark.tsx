/*
 * The mark this server wears: the logo its administrator gave it, or
 * Melyxar's own in the accent when there is none. One place deciding, so no
 * screen shows Melyxar's where somebody put their own.
 *
 * Nothing until the server has said which, so Melyxar's never flashes up in
 * front of somebody else's. Decorative: the server's name is always written
 * next to it.
 */

import type { ServerIdentity } from "../api";
import { MelyxarMark } from "../icons";

export function ServerMark({
  branding,
  size,
  className,
  logoClassName,
}: {
  branding: ServerIdentity | null;
  size: number;
  /** Worn by either. */
  className?: string;
  /** Worn by the administrator's logo only, which keeps its own shape. */
  logoClassName?: string;
}) {
  if (!branding) {
    return null;
  }
  if (branding.logo) {
    const classes = [className, logoClassName].filter(Boolean).join(" ");
    return <img className={classes || undefined} src={branding.logo} alt="" aria-hidden="true" />;
  }
  return <MelyxarMark size={size} className={className} />;
}
