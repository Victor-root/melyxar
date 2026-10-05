import { Link, NavLink } from "react-router-dom";
import { BellIcon, TickIcon } from "../icons";
import { Dropdown } from "../components/dropdown";
import { sayPoint } from "../pages/admin/activity";
import { JobCard } from "../pages/admin/tasks";
import { useRunning } from "../running";
import { useSettings } from "../settings";
import type { Level } from "./api";
import { pieOf } from "./pie";
import { useAttention } from "./attention";
import { NoteList } from "./list";
import { useNotes } from "./store";

/**
 * How many things wait, and the kinds among them: the unread notifications,
 * and for an administrator the points deserving a look.
 */
function useWaiting(administrator: boolean): { count: number; levels: Level[]; working: boolean } {
  const { notes, unread } = useNotes();
  const { points } = useAttention();
  const { jobs } = useRunning();
  const looked = administrator ? (points ?? []) : [];
  const levels: Level[] = [
    ...notes.filter((note) => !note.read).map((note) => note.level),
    ...looked.map((point) => point.state),
  ];
  return { count: unread + looked.length, levels, working: administrator && jobs.length > 0 };
}

/** Said on the bell while the server is at work, which only an administrator
 *  is told: a dot, apart from the count, since work is not something to be
 *  read but something going on. */
function Busy() {
  return <span className="bell-busy" aria-hidden="true" />;
}

/** The number of things that wait, on a background of the colour of what
 *  waits, or shared between the colours when there are several. */
function Count({ count, levels, line }: { count: number; levels: Level[]; line?: boolean }) {
  return (
    <span className={`bell-count${line ? " bell-count-line" : ""}`} style={{ background: pieOf(levels) }}>
      {count}
    </span>
  );
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
  const { count, levels, working } = useWaiting(administrator);
  const { jobs } = useRunning();
  const looked = administrator ? (points ?? []) : [];

  return (
    <Dropdown
      className="header-bell"
      icon={t("nav.notifications")}
      listClassName="bell-list"
      /* Looked at, so read: what was new is told by the count that goes, and
         any of them can be put back as unread. */
      onOpen={() => {
        if (unread > 0) markRead();
      }}
      reachable
      label={
        <>
          <BellIcon size={24} />
          {working && <Busy />}
          {count > 0 && <Count count={count} levels={levels} />}
        </>
      }
    >
      {/* The work the server is doing, each piece as a notification of its own
          with its bar, which fills in as the work does. Only for an
          administrator, who is the one it concerns. */}
      {working && (
        <>
          <span className="bell-head">
            <span className="bell-title">{t("jobs.running")}</span>
          </span>
          {jobs.map((job) => (
            <Link key={job.id} to="/admin/tasks" className="header-menu-line bell-job">
              <JobCard job={job} />
            </Link>
          ))}
        </>
      )}
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
  const { count, levels, working } = useWaiting(administrator);
  return (
    <NavLink to="/notifications" className="header-menu-line">
      <BellIcon size={16} />
      {t("nav.notifications")}
      {working && <Busy />}
      {count > 0 && <Count count={count} levels={levels} line />}
    </NavLink>
  );
}
