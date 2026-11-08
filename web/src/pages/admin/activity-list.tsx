/*
 * Lines of the activity journal, as the administration shows them.
 *
 * One list for the page of the journal, the history of what was watched and
 * the summary, so a line reads the same wherever it is met. The newest lines
 * are asked for again the moment the server says one was written, so what
 * happens shows up without anybody reloading; older ones are fetched when
 * asked for, and kept.
 */

import { useCallback, useEffect, useState } from "react";
import type { ComponentType } from "react";
import { api } from "../../api";
import type { ActivityAccount, ActivityFamily, ActivityLine, ActivityPage } from "../../api";
import { wasAbandoned } from "../../asking";
import {
  FolderIcon,
  LockIcon,
  PlaybackIcon,
  ServerIcon,
  WarningIcon,
} from "../../icons";
import type { IconProps } from "../../icons";
import { useJournalNews } from "../../live";
import { useSettings } from "../../settings";
import { Face } from "../../components/face";
import { familyOf, sayLine, whenItHappened } from "./activity";

const FAMILY_ICONS: Record<ActivityFamily, ComponentType<IconProps>> = {
  access: LockIcon,
  playback: PlaybackIcon,
  library: FolderIcon,
  server: ServerIcon,
  refused: WarningIcon,
};

export const FAMILIES: ActivityFamily[] = [
  "access",
  "playback",
  "library",
  "server",
];

interface Following {
  lines: ActivityLine[];
  more: boolean;
  failed: boolean;
  /** Fetches the lines older than the last one shown. */
  older: () => void;
}

/** Counted up each time the server says a line was written. */
function useWritten(): number {
  const [written, setWritten] = useState(0);
  useJournalNews(() => setWritten((count) => count + 1));
  return written;
}

/**
 * The lines of these families, about these accounts or about anybody when
 * none is, newest first: the newest page asked for again whenever a line is
 * written, the older ones kept once fetched.
 */
export function useActivity(
  families: ActivityFamily[],
  most?: number,
  users: string[] = [],
): Following {
  const [head, setHead] = useState<ActivityPage | null>(null);
  const [tail, setTail] = useState<ActivityPage | null>(null);
  const [failed, setFailed] = useState(false);
  const asked = families.join(",");
  const whom = users.join(",");
  const written = useWritten();

  useEffect(() => {
    setHead(null);
    setTail(null);
  }, [asked, whom, most]);

  useEffect(() => {
    const controller = new AbortController();
    const wanted = asked === "" ? [] : (asked.split(",") as ActivityFamily[]);
    api
      .activity(wanted, null, controller.signal, most, whom === "" ? [] : whom.split(","))
      .then((page) => {
        setHead(page);
        setFailed(false);
      })
      .catch((error) => {
        if (!wasAbandoned(error)) {
          setFailed(true);
        }
      });
    return () => controller.abort();
  }, [asked, whom, most, written]);

  // The newest page, then whatever was fetched after it that is older than
  // its last line: a line written meanwhile moves the head, never the tail.
  const headLines = head?.lines ?? [];
  const last = headLines[headLines.length - 1]?.id ?? null;
  const tailLines = (tail?.lines ?? []).filter(
    (line) => last === null || line.id < last,
  );
  const lines = [...headLines, ...tailLines];
  const more = tail ? tail.more : (head?.more ?? false);

  const oldest = lines[lines.length - 1]?.id ?? null;
  const older = useCallback(() => {
    if (oldest === null) {
      return;
    }
    const wanted = asked === "" ? [] : (asked.split(",") as ActivityFamily[]);
    api
      .activity(wanted, oldest, undefined, most, whom === "" ? [] : whom.split(","))
      .then((page) =>
        setTail((was) => ({
          lines: [...(was?.lines ?? []), ...page.lines],
          more: page.more,
        })),
      )
      .catch(() => setFailed(true));
  }, [asked, whom, oldest, most]);

  return { lines, more, failed, older };
}

/** The lines themselves, each with who or what it is about, what happened
 *  and when. A line about somebody wears their face, which is what the eye
 *  looks for first down a list of who did what; one about the server or a
 *  library, or about an account since removed, wears the mark of its
 *  family. */
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
        const state =
          line.level === "information" ? "" : ` state-${line.level}`;
        return (
          <div className="line activity-line" key={line.id}>
            {line.user_id ? (
              <span className={`line-mark line-mark-face${state}`}>
                <Face
                  avatar={line.user_avatar}
                  picture={line.user_face_picture}
                  className="account-face"
                />
              </span>
            ) : (
              <span className={`line-mark${state}`} aria-hidden="true">
                <FamilyIcon size={17} />
              </span>
            )}
            <span className="line-words">
              <span className="line-name">{said.title}</span>
              {said.note && <span className="line-note">{said.note}</span>}
            </span>
            <span
              className="line-end"
              title={new Date(line.at).toLocaleString(language)}
            >
              {whenItHappened(line.at, now, language, t)}
            </span>
          </div>
        );
      })}
    </div>
  );
}

/** The accounts the lines of these families are about, read again as the
 *  journal grows. */
function useAccounts(families: ActivityFamily[]): ActivityAccount[] {
  const [accounts, setAccounts] = useState<ActivityAccount[]>([]);
  const asked = families.join(",");
  const written = useWritten();
  useEffect(() => {
    const controller = new AbortController();
    api
      .activityAccounts(asked === "" ? [] : (asked.split(",") as ActivityFamily[]), controller.signal)
      .then(setAccounts)
      .catch(() => {
        // The chips there were stay: the lines still answer.
      });
    return () => controller.abort();
  }, [asked, written]);
  return accounts;
}

/** A chip for each account the journal speaks of, as the technical journal
 *  has one for each of its tags: any number of them ticked narrows it to
 *  them, none shows everybody. */
function AccountChips({
  families,
  ticked,
  onTicked,
}: {
  families: ActivityFamily[];
  ticked: string[];
  onTicked: (ticked: string[]) => void;
}) {
  const { t } = useSettings();
  const accounts = useAccounts(families);
  if (accounts.length < 2) {
    return null;
  }
  return (
    <div className="chips activity-accounts">
      <button
        className={`chip${ticked.length === 0 ? " chip-on" : ""}`}
        aria-pressed={ticked.length === 0}
        onClick={() => onTicked([])}
      >
        {t("activity.everybody")}
      </button>
      {accounts.map((account) => {
        const on = ticked.includes(account.id);
        return (
          <button
            key={account.id}
            className={`chip${on ? " chip-on" : ""}`}
            aria-pressed={on}
            onClick={() => onTicked(on ? ticked.filter((id) => id !== account.id) : [...ticked, account.id])}
          >
            <Face avatar={account.avatar} picture={account.face_picture} className="chip-face" />
            {account.name}
            <span className="chip-count">{account.lines}</span>
          </button>
        );
      })}
    </div>
  );
}

/** The whole journal of some families, and the button that goes further
 *  back in it; narrowed to some accounts by their chips [byAccount]. */
export function ActivityJournal({
  families,
  byAccount = false,
}: {
  families: ActivityFamily[];
  byAccount?: boolean;
}) {
  const { t } = useSettings();
  const [ticked, setTicked] = useState<string[]>([]);
  const { lines, more, failed, older } = useActivity(families, undefined, ticked);
  return (
    <>
      {byAccount && <AccountChips families={families} ticked={ticked} onTicked={setTicked} />}
      {failed && (
        <p className="panel-notice panel-notice-trouble">
          {t("error.unreachable")}
        </p>
      )}
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
