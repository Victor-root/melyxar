/*
 * The outline of a table not filled yet: its column heads, and two rows of
 * nothing under them, so a page drawn ahead of its engine shows its shape.
 */

import { useSettings } from "../../settings";

export function Ghosts({ heads }: { heads: string[] }) {
  const { t } = useSettings();
  return (
    <div className="ghosts" style={{ ["--columns" as string]: heads.length }}>
      {heads.map((head) => (
        <span key={head} className="ghosts-head">
          {t(head)}
        </span>
      ))}
      {[0, 1].flatMap((row) =>
        heads.map((head) => <span key={`${row}-${head}`} className="ghosts-cell" />),
      )}
    </div>
  );
}
