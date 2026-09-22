/*
 * The way back up from a work to what it hangs under.
 *
 * Drawn before anything else on a page, out of what came with the page rather
 * than out of a second question: a heading that arrives late is a page that
 * jumps.
 */

import { Link } from "react-router-dom";
import type { Work } from "../api";
import { nameOfOne } from "../readable";
import { useSettings } from "../settings";

export function WayBackUp({ work }: { work: Work }) {
  const { t } = useSettings();
  if (work.ancestry.length === 0) {
    return null;
  }
  return (
    <nav className="work-ancestry">
      {[...work.ancestry].reverse().map((up) => (
        <Link key={up.id} to={`/work/${up.id}`} className="work-ancestor">
          {nameOfOne(up.kind, up.number, up.title, t) || up.title}
        </Link>
      ))}
    </nav>
  );
}
