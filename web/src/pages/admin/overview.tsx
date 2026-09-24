/*
 * The summary: the server at a glance, and a way into every part of it.
 *
 * What the server can already say is said: whether it is up and for how long,
 * its libraries, the work it is doing, who uses it. What it cannot say yet is
 * drawn where it will go, asleep and saying so, so that the page is the page
 * it is going to be.
 */

import { useEffect, useState } from "react";
import type { ComponentType, ReactNode } from "react";
import { Link } from "react-router-dom";
import { api } from "../../api";
import type { ActivityFamily, Job, Library, Overview } from "../../api";
import { useAsked } from "../../asking";
import { PageHead, Panel, Soon, Stat, StatePill } from "../../components/panel";
import type { State } from "../../components/panel";
import {
  ArrowRightIcon,
  ClockIcon,
  DatabaseIcon,
  DeviceIcon,
  FfmpegIcon,
  FolderIcon,
  GraphicsCardIcon,
  KindIcon,
  MelyxarMark,
  PeopleIcon,
  TasksIcon,
  WarningIcon,
} from "../../icons";
import type { IconProps } from "../../icons";
import { useLibraries } from "../../libraries";
import { howLongSince, howMany, outOfAHundred, releaseOf } from "../../readable";
import { useRunning } from "../../running";
import { useAttention } from "../../attention";
import { useSettings } from "../../settings";
import { sayPoint, sayWorry } from "./activity";
import { ActivityLines, useActivity } from "./activity-list";
import { useOverview } from "./layout";
import { SystemPanel } from "./machine";
import { PlayingPanel } from "./playback";

/** Every family of the journal, for the lines of the summary. */
const EVERY_FAMILY: ActivityFamily[] = [];

/** How many of the latest lines the summary shows. */
const RECENT_LINES = 6;

/** How often the time the server has been up is written again. */
const A_MINUTE_MS = 60_000;

export function AdminOverview() {
  const { t } = useSettings();
  const overview = useOverview();

  return (
    <>
      <PageHead lead={t("admin.overview_lead")} />

      {/* What is playing and what just happened, one panel: the second is
          mostly the first, finished. */}
      <PlayingPanel>
        <hr className="part-rule" />
        <RecentActivity />
        <div className="panel-foot">
          <Link className="button button-small" to="/admin/journal">
            {t("activity.see_journal")}
            <ArrowRightIcon size={15} />
          </Link>
          <Link className="button button-small button-accent" to="/admin/playback">
            {t("admin.see_playback")}
            <ArrowRightIcon size={15} />
          </Link>
        </div>
      </PlayingPanel>

      <div className="overview-panels">
        <SystemPanel card={overview.answer?.media_tools.card ?? null} />
        <ServerPanel overview={overview.answer} unreachable={overview.failure !== null} />
        <LibrariesPanel />
        <TasksPanel />
        <WatchPanel />
        <PeoplePanel overview={overview.answer} />
      </div>
    </>
  );
}

/** The time now, moved on once a minute, for figures counted from it. */
function useMinute(): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), A_MINUTE_MS);
    return () => window.clearInterval(timer);
  }, []);
  return now;
}

/**
 * The server itself, beside what the machine spends: its name, whether all is
 * well, and the handful of facts every other answer rests on.
 */
function ServerPanel({ overview, unreachable }: { overview: Overview | null; unreachable: boolean }) {
  const { t, language } = useSettings();
  const now = useMinute();

  const worries = overview?.worries ?? [];
  const state: State = unreachable ? "trouble" : worries.length > 0 ? "attention" : "ok";

  return (
    <section className="panel server-panel">
      <div className="server-who">
        <span className="server-mark" aria-hidden="true">
          <MelyxarMark size={36} />
        </span>
        <div className="server-words">
          <h2>{overview?.server_name ?? t("app.name")}</h2>
          <StatePill state={state}>
            {t(unreachable ? "admin.unreachable" : "admin.online")}
          </StatePill>
          <p>
            {unreachable
              ? t("admin.unreachable_why")
              : worries.length > 0
                ? howMany(worries.length, "admin.worries", t)
                : t("admin.all_fine")}
          </p>
          {/* Each point that failed its check, by name: a count alone sends
              somebody hunting for what it counts. */}
          {!unreachable && worries.length > 0 && (
            <ul className="server-worries">
              {worries.map((worry, index) => (
                <li key={index}>
                  <WarningIcon size={15} />
                  {sayWorry(worry, t, language)}
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>

      <div className="server-facts">
        <Fact
          icon={MelyxarMark}
          label={t("admin.version")}
          value={overview && releaseOf(overview.version)}
          title={overview?.version}
        />
        <Fact
          icon={ClockIcon}
          label={t("admin.up_for")}
          value={overview && howLongSince(overview.started_at, now, t)}
        />
        <Fact
          icon={DatabaseIcon}
          label={t("admin.database")}
          value={overview && t(overview.database_ready ? "admin.ready" : "admin.to_check")}
          state={overview ? (overview.database_ready ? "ok" : "trouble") : undefined}
        />
        <Fact
          icon={FfmpegIcon}
          label={t("admin.media_tools")}
          value={overview && t(overview.media_tools.found ? "admin.found" : "admin.missing")}
          state={overview ? (overview.media_tools.found ? "ok" : "trouble") : undefined}
        />
        <Fact
          icon={GraphicsCardIcon}
          label={t("admin.card")}
          value={
            overview &&
            (overview.media_tools.card
              ? overview.media_tools.card_opens
                ? overview.media_tools.card.toUpperCase()
                : t("admin.card_closed")
              : t("admin.card_unused"))
          }
          state={
            overview
              ? overview.media_tools.card
                ? overview.media_tools.card_opens
                  ? "ok"
                  : "trouble"
                : "attention"
              : undefined
          }
        />
      </div>

      <div className="panel-foot">
        <Link className="button button-small button-accent" to="/admin/diagnostics">
          {t("admin.see_diagnostics")}
          <ArrowRightIcon size={15} />
        </Link>
      </div>
    </section>
  );
}

function Fact({
  icon: FactIcon,
  label,
  value,
  state,
  title,
}: {
  icon: ComponentType<IconProps>;
  label: string;
  value: ReactNode;
  state?: State;
  /** The whole of what is shown shortened, for whoever points at it. */
  title?: string;
}) {
  return (
    <div className="fact" title={title}>
      <span className="fact-mark" aria-hidden="true">
        <FactIcon size={20} />
      </span>
      <span className="fact-words">
        <span className="fact-label">{label}</span>
        <span className="fact-value">
          {value ?? "–"}
          {state && <span className={`state-dot state-${state}`} aria-hidden="true" />}
        </span>
      </span>
    </div>
  );
}

/** Every library with what it holds, and the scan under way. */
function LibrariesPanel() {
  const { t, language } = useSettings();
  const { all: libraries } = useLibraries();
  const { jobs } = useRunning();
  const scanning = jobs.find((job) => job.kind === "scan");
  const hasMusic = libraries.some((library) => library.kind === "music");

  return (
    <Panel icon={FolderIcon} title={t("admin.libraries")} lead={t("admin.libraries_lead")}>
      <div className="lines">
        {libraries.length === 0 && <p className="empty-line">{t("admin.no_library")}</p>}
        {libraries.map((library) => (
          <LibraryLine key={library.id} library={library} language={language} />
        ))}
        {!hasMusic && (
          <div className="line line-asleep">
            <span className="line-mark" aria-hidden="true">
              <KindIcon kind="music" size={18} />
            </span>
            <span className="line-words">
              <span className="line-name">{t("kind.music")}</span>
            </span>
            <span className="line-end">
              <Soon />
            </span>
          </div>
        )}
      </div>

      {scanning && <WorkUnderWay job={scanning} />}

      <div className="panel-foot">
        <Link className="button button-small button-accent" to="/admin/libraries">
          {t("admin.manage_libraries")}
          <ArrowRightIcon size={15} />
        </Link>
      </div>
    </Panel>
  );
}

function LibraryLine({ library, language }: { library: Library; language: string }) {
  const { t } = useSettings();
  return (
    <div className="line">
      <span className="line-mark" aria-hidden="true">
        <KindIcon kind={library.kind} size={18} />
      </span>
      <span className="line-words">
        <span className="line-name">{library.name}</span>
        <span className="line-note">{t(`kind.${library.kind}`)}</span>
      </span>
      <span className="line-end">
        <span className="line-figure">{library.works.toLocaleString(language)}</span>
        {t(library.works === 1 ? "admin.titles_one" : "admin.titles")}
      </span>
    </div>
  );
}

/** One piece of work, with the bar of how far it has gone. */
function WorkUnderWay({ job }: { job: Job }) {
  const { t } = useSettings();
  return (
    <div className="under-way">
      <span className="under-way-words">
        <span>{t(`jobs.${job.kind}`)}</span>
        {job.step && <span className="line-note">{t(`jobs.step.${job.step}`)}</span>}
        {job.ratio !== null && (
          <span className="under-way-share">{outOfAHundred(job.ratio)} %</span>
        )}
      </span>
      <span className="meter">
        <span
          className="meter-fill"
          style={{ width: `${job.ratio === null ? 0 : outOfAHundred(job.ratio)}%` }}
        />
      </span>
    </div>
  );
}

/** What the server is doing on its own, and when it next will. */
function TasksPanel() {
  const { t, language } = useSettings();
  const { jobs, finished } = useRunning();
  const upkeep = useAsked((signal) => api.upkeep(signal), [finished]);
  const next = upkeep.answer?.next_run ?? null;

  return (
    <Panel icon={TasksIcon} title={t("admin.tasks")} lead={t("admin.tasks_lead")}>
      {jobs.length === 0 ? (
        <p className="empty-line">{t("jobs.none")}</p>
      ) : (
        jobs.map((job) => <WorkUnderWay key={job.id} job={job} />)
      )}

      <div className="lines">
        <div className="line">
          <span className="line-mark" aria-hidden="true">
            <ClockIcon size={18} />
          </span>
          <span className="line-words">
            <span className="line-name">{t("upkeep.title")}</span>
            <span className="line-note">
              {upkeep.answer &&
                (next
                  ? t("admin.upkeep_next", {
                      when: new Date(next).toLocaleTimeString(language, {
                        hour: "2-digit",
                        minute: "2-digit",
                      }),
                    })
                  : t("upkeep.nightly_off"))}
            </span>
          </span>
        </div>
      </div>

      <div className="panel-foot">
        <Link className="button button-small button-accent" to="/admin/tasks">
          {t("admin.see_tasks")}
          <ArrowRightIcon size={15} />
        </Link>
      </div>
    </Panel>
  );
}

/** What deserves a look, gathered from everywhere, each leading to where it
 *  is put right. The same points as the bell, and quieted with it. */
function WatchPanel() {
  const { t, language } = useSettings();
  const { points, markSeen } = useAttention();
  const shown = points ?? [];

  return (
    <Panel icon={WarningIcon} title={t("admin.watch")} lead={t("admin.watch_lead")}>
      {points !== null && shown.length === 0 && (
        <p className="empty-line">{t("attention.none")}</p>
      )}
      <div className="lines">
        {shown.map((point, index) => {
          const said = sayPoint(point, t, language);
          return (
            <Link className="line line-link" to={said.to} key={index}>
              <span className={`line-mark state-${point.state}`} aria-hidden="true">
                <WarningIcon size={18} />
              </span>
              <span className="line-words">
                <span className="line-name">{said.title}</span>
              </span>
              <span className="line-end">
                <ArrowRightIcon size={15} />
              </span>
            </Link>
          );
        })}
      </div>
      {shown.some((point) => point.may_be_seen) && (
        <div className="panel-foot">
          <button className="button button-small" onClick={() => void markSeen()}>
            {t("attention.mark_seen")}
          </button>
        </div>
      )}
    </Panel>
  );
}

/** How many people use this server, and from how many things. */
function PeoplePanel({ overview }: { overview: Overview | null }) {
  const { t, language } = useSettings();
  const count = (value: number | undefined) =>
    value === undefined ? "–" : value.toLocaleString(language);

  return (
    <Panel icon={PeopleIcon} title={t("admin.people")} lead={t("admin.people_lead")}>
      <div className="stats stats-three">
        <Stat icon={PeopleIcon} label={t("admin.accounts")} value={count(overview?.accounts)} />
        <Stat icon={DeviceIcon} label={t("admin.devices")} value={count(overview?.devices)} />
        <Stat
          icon={ClockIcon}
          label={t("admin.active_today")}
          value={count(overview?.devices_today)}
        />
      </div>
      <div className="panel-foot">
        <Link className="button button-small button-accent" to="/admin/users">
          {t("admin.see_users")}
          <ArrowRightIcon size={15} />
        </Link>
      </div>
    </Panel>
  );
}

/** What happened lately, across the whole server. */
/** The latest lines of the journal, under what is playing. */
function RecentActivity() {
  const { t } = useSettings();
  const { lines } = useActivity(EVERY_FAMILY, RECENT_LINES);
  return (
    <section>
      <h3 className="part-title">{t("admin.recent")}</h3>
      <ActivityLines lines={lines} />
    </section>
  );
}
