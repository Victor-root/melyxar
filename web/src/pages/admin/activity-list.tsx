/*
 * Lines of the activity journal, as the administration shows them.
 *
 * One list for the page of the journal, the history of what was watched and
 * the summary, so a line reads the same wherever it is met. The newest lines
 * are asked for again every ten seconds, so what happens shows up without
 * anybody reloading; older ones are fetched when asked for, and kept.
 */

import { useCallback, useEffect, useState } from "react";
import type { ComponentType } from "react";
import { api } from "../../api";
import type { ActivityFamily, ActivityLine, ActivityPage } from "../../api";
import { wasAbandoned } from "../../asking";
import { FolderIcon, LockIcon, PlaybackIcon, ServerIcon } from "../../icons";
import type { IconProps } from "../../icons";
import { useSettings } from "../../settings";
import { familyOf, sayLine, whenItHappened } from "./activity";

/** How often the newest lines are asked for again. */
const LOOKED_AT_EVERY_MS = 10_000;

const FAMILY_ICONS: Record<ActivityFamily, ComponentType<IconProps>> = {
  access: LockIcon,
  playback: PlaybackIcon,
  library: FolderIcon,
  server: ServerIcon,
};

export const FAMILIES: ActivityFamily[] = ["access", "playback", "library", "server"];

interface Following {
  lines: ActivityLine[];
  more: boolean;
  failed: boolean;
  /** Fetches the lines older than the last one shown. */
  older: () => void;
}

/**
 * The lines of these families, newest first: the newest page asked for again
 * on a beat, the older ones kept once fetched.
 */
export function useActivity(families: ActivityFamily[], most?: number): Following {
  const [head, setHead] = useState<ActivityPage | null>(null);
  const [tail, setTail] = useState<ActivityPage | null>(null);
  const [failed, setFailed] = useState(false);
  const asked = families.join(",");

  useEffect(() => {
    const controller = new AbortController();
    const wanted = asked === "" ? [] : (asked.split(",") as ActivityFamily[]);
    setHead(null);
    setTail(null);
    const look = () =>
      api
        .activity(wanted, null, controller.signal, most)
        .then((page) => {
          setHead(page);
          setFailed(false);
        })
        .catch((error) => {
          if (!wasAbandoned(error)) {
            setFailed(true);
          }
        });
    void look();
    const timer = window.setInterval(look, LOOKED_AT_EVERY_MS);
    return () => {
      window.clearInterval(timer);
      controller.abort();
    };
  }, [asked, most]);

  // The newest page, then whatever was fetched after it that is older than
  // its last line: a line written meanwhile moves the head, never the tail.
  const headLines = head?.lines ?? [];
  const last = headLines[headLines.length - 1]?.id ?? null;
  const tailLines = (tail?.lines ?? []).filter((line) => last === null || line.id < last);
  const lines = [...headLines, ...tailLines];
  const more = tail ? tail.more : (head?.more ?? false);

  const oldest = lines[lines.length - 1]?.id ?? null;
  const older = useCallback(() => {
    if (oldest === null) {
      return;
    }
    const wanted = asked === "" ? [] : (asked.split(",") as ActivityFamily[]);
    api
      .activity(wanted, oldest, undefined, most)
      .then((page) =>
        setTail((was) => ({ lines: [...(was?.lines ?? []), ...page.lines], more: page.more })),
      )
      .catch(() => setFailed(true));
  }, [asked, oldest, most]);

  return { lines, more, failed, older };
}

/** The lines themselves, each with its family, what happened and when. */
export function ActivityLines({ lines }: { lines: ActivityLine[] }) {
  const { t, language } = useSettings();
  const now = new Date();

  if (lines.length === 0) {
    return <p className="empty-line">{t("activity.none")}</p>;
  }
  return (
    <div className="lines">
      {lines.map((line) => {
        const said = sayLine(line, t);
        const FamilyIcon = FAMILY_ICONS[familyOf(line.kind)];
        return (
          <div className="line activity-line" key={line.id}>
            <span
              className={`line-mark${line.level === "information" ? "" : ` state-${line.level}`}`}
              aria-hidden="true"
            >
              <FamilyIcon size={17} />
            </span>
            <span className="line-words">
              <span className="line-name">{said.title}</span>
              {said.note && <span className="line-note">{said.note}</span>}
            </span>
            <span className="line-end" title={new Date(line.at).toLocaleString(language)}>
              {whenItHappened(line.at, now, language, t)}
            </span>
          </div>
        );
      })}
    </div>
  );
}

/** The whole journal of some families, and the button that goes further
 *  back in it. */
export function ActivityJournal({ families }: { families: ActivityFamily[] }) {
  const { t } = useSettings();
  const { lines, more, failed, older } = useActivity(families);
  return (
    <>
      {failed && <p className="panel-notice panel-notice-trouble">{t("error.unreachable")}</p>}
      <ActivityLines lines={lines} />
      {more && (
        <div className="panel-foot">
          <button className="button button-small" onClick={older}>
            {t("activity.older")}
          </button>
        </div>
      )}
    </>
  );
}
