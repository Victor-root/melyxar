import { useRef } from "react";
import { Link, NavLink } from "react-router-dom";
import { BellIcon, CloseIcon, TickIcon } from "../icons";
import { PHONE, useMediaQuery } from "../media-query";
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

/** How far up the handle has to be pulled to put the sheet away, at most: a
 *  short sheet goes with a third of its height. */
const PULL_TO_CLOSE = 120;

/** How far a finger may wander on the handle and still be pressing it. */
const PRESS_SLACK = 6;

/** How long the sheet takes to go, or to come back, once let go of. */
const LETS_GO_MS = 180;

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
  const onAPhone = useMediaQuery(PHONE);
  /* The pull in progress on the handle: where the finger went down and the
     sheet it is moving. */
  const pulled = useRef<{ from: number; sheet: HTMLElement; moved: boolean } | null>(null);

  return (
    <Dropdown
      className="header-bell"
      icon={t("nav.notifications")}
      listClassName="bell-list"
      placed={!onAPhone}
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
      {/* The title of the sheet a phone opens, with the way out: not drawn
          anywhere else. */}
      <div className="bell-sheet-head">
        <span className="bell-sheet-icon">
          <BellIcon size={20} />
        </span>
        <span className="bell-sheet-title">{t("nav.notifications")}</span>
        <button type="button" className="bell-sheet-close" aria-label={t("modal.close")}>
          <CloseIcon size={18} />
        </button>
      </div>
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
      {/* The handle at the foot of the sheet: held and pulled up, the sheet
          follows the finger and goes if it was pulled far enough; pressed, it
          goes at once, as any press in the list does. */}
      <button
        type="button"
        className="bell-handle"
        aria-label={t("modal.close")}
        onPointerDown={(event) => {
          const sheet = event.currentTarget.closest<HTMLElement>(".bell-list");
          if (!sheet) {
            return;
          }
          event.currentTarget.setPointerCapture(event.pointerId);
          sheet.style.animation = "none";
          sheet.style.transition = "none";
          pulled.current = { from: event.clientY, sheet, moved: false };
        }}
        onPointerMove={(event) => {
          const pull = pulled.current;
          if (!pull) {
            return;
          }
          const up = Math.min(0, event.clientY - pull.from);
          pull.moved = pull.moved || up < -PRESS_SLACK;
          pull.sheet.style.transform = `translateY(${up}px)`;
        }}
        onPointerUp={(event) => {
          const pull = pulled.current;
          if (!pull) {
            return;
          }
          const handle = event.currentTarget;
          const up = event.clientY - pull.from;
          pull.sheet.style.transition = `transform ${LETS_GO_MS}ms ease-out`;
          if (up < -Math.min(PULL_TO_CLOSE, pull.sheet.offsetHeight * 0.3)) {
            pull.sheet.style.transform = "translateY(-100%)";
            /* The click the browser sends after this is the end of the pull
               and is swallowed; the one that shuts the sheet is sent once it
               has gone. */
            pull.moved = true;
            window.setTimeout(() => {
              pulled.current = null;
              handle.click();
            }, LETS_GO_MS);
            return;
          }
          pull.sheet.style.transform = "";
        }}
        onPointerCancel={() => {
          const pull = pulled.current;
          pulled.current = null;
          if (pull) {
            pull.sheet.style.transition = `transform ${LETS_GO_MS}ms ease-out`;
            pull.sheet.style.transform = "";
          }
        }}
        onClick={(event) => {
          /* The press that ends a pull is not a press on the handle. */
          const pull = pulled.current;
          pulled.current = null;
          if (pull?.moved) {
            event.stopPropagation();
          }
        }}
      >
        <span aria-hidden="true" />
      </button>
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
