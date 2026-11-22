/*
 * The press that marks one point deserving a look as seen, beside the point
 * in the bell and on the summary, as a notification is marked as read
 * beside it.
 */

import type { MouseEvent } from "react";
import { EyeIcon } from "../icons";
import { useSettings } from "../settings";
import { useAttention } from "./attention";

export function PointSeen({ point }: { point: string }) {
  const { t } = useSettings();
  const { markSeen } = useAttention();
  return (
    <button
      type="button"
      className="note-action"
      title={t("attention.mark_one_seen")}
      aria-label={t("attention.mark_one_seen")}
      onClick={(event: MouseEvent) => {
        // The list it sits in stays open, the way a notification marked as
        // read leaves it.
        event.stopPropagation();
        void markSeen(point);
      }}
    >
      <EyeIcon size={15} />
    </button>
  );
}
