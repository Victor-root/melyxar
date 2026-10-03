import { Link, NavLink } from "react-router-dom";
import { BellIcon, TickIcon } from "../icons";
import { Dropdown } from "../components/dropdown";
import { sayPoint } from "../pages/admin/activity";
import { useSettings } from "../settings";
import type { Level } from "./api";
import { useAttention } from "./attention";
import { NoteList } from "./list";
import { useNotes } from "./store";

/** The order of the colours, the gravest last. */
const GRAVITY: Level[] = ["ok", "news", "attention", "trouble"];

/**
 * How many things wait, and the gravest colour among them: the unread
 * notifications, and for an administrator the points deserving a look.
 */
function useWaiting(administrator: boolean): { count: number; worst: Level } {
  const { notes, unread } = useNotes();
  const { points } = useAttention();
  const looked = administrator ? (points ?? []) : [];
  const levels: Level[] = [
    ...notes.filter((note) => !note.read).map((note) => note.level),
    ...looked.map((point) => point.state),
  ];
  const worst = levels.reduce<Level>(
    (gravest, level) => (GRAVITY.indexOf(level) > GRAVITY.indexOf(gravest) ? level : gravest),
    "ok",
  );
  return { count: unread + looked.length, worst };
}

/**
 * The bell of the bar: how many things wait, in the colour of the gravest,
 * and on a press the points deserving an administrator's look, then this
 * account's notifications.
 */
export function Bell({ administrator }: { administrator: boolean }) {
  const { t, language } = useSettings();
  const { points, markSeen } = useAttention();
  const { unread, markRead } = useNotes();
  const { count, worst } = useWaiting(administrator);
  const looked = administrator ? (points ?? []) : [];

  return (
    <Dropdown
      className="header-bell"
      icon={t("nav.notifications")}
      listClassName="bell-list"
      reachable
      label={
        <>
          <BellIcon size={24} />
          {count > 0 && <span className={`bell-count bell-count-${worst}`}>{count}</span>}
        </>
      }
    >
      {looked.length > 0 && (
        <>
          {/* Marking them seen sits with the title rather than under the
              last point, where a long list would push it out of reach. */}
          <span className="bell-head">
            <span className="bell-title">{t("admin.watch")}</span>
            {looked.some((point) => point.may_be_seen) && (
              <button type="button" className="bell-seen" onClick={() => void markSeen()}>
                <TickIcon size={14} />
                {t("attention.mark_seen")}
              </button>
            )}
          </span>
          {looked.map((point, index) => {
            const said = sayPoint(point, t, language);
            return (
              <Link key={index} to={said.to} className="header-menu-line bell-point">
                <span className={`state-dot state-${point.state}`} aria-hidden="true" />
                <span>{said.title}</span>
              </Link>
            );
          })}
        </>
      )}
      <span className="bell-head">
        <span className="bell-title">{t("nav.notifications")}</span>
        {unread > 0 && (
          <button
            type="button"
            className="bell-seen"
            onClick={(event) => {
              event.stopPropagation();
              markRead();
            }}
          >
            <TickIcon size={14} />
            {t("notes.mark_all_read")}
          </button>
        )}
      </span>
      <NoteList />
      <Link to="/notifications" className="bell-all">
        {t("notes.see_all")}
      </Link>
    </Dropdown>
  );
}

/**
 * The bell moved into the account's menu: the page of the history, with how
 * many things wait.
 */
export function BellLine({ administrator }: { administrator: boolean }) {
  const { t } = useSettings();
  const { count, worst } = useWaiting(administrator);
  return (
    <NavLink to="/notifications" className="header-menu-line">
      <BellIcon size={16} />
      {t("nav.notifications")}
      {count > 0 && <span className={`bell-count bell-count-${worst} bell-count-line`}>{count}</span>}
    </NavLink>
  );
}
