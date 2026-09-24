/*
 * The activity journal and what deserves a look, said in words.
 *
 * The server writes what happened as facts: a kind, names, figures. The
 * sentences are this side's, so they follow the language of whoever reads
 * them, and so that one line reads the same on the summary, on the page of
 * the journal and in the history of what was watched.
 */

import type { ActivityFamily, ActivityLine, AttentionPoint } from "../../api";
import { deviceName } from "../../devices";
import { howLong, howMany, percentOf, releaseOf } from "../../readable";
import type { Wording } from "../../readable";

/** The family a kind of line belongs to, for its icon and its filter. */
export function familyOf(kind: string): ActivityFamily {
  if (kind === "watched") {
    return "playback";
  }
  if (kind.startsWith("task_") || kind === "works_deleted") {
    return "library";
  }
  if (kind.startsWith("server_")) {
    return "server";
  }
  return "access";
}

/** A line in words: what happened, and what else is worth knowing. */
export interface Said {
  title: string;
  note: string | null;
}

function text(details: Record<string, unknown>, key: string): string | null {
  const value = details[key];
  return typeof value === "string" && value !== "" ? value : null;
}

function figure(details: Record<string, unknown>, key: string): number | null {
  const value = details[key];
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

/** A length of time from a count of seconds, as somebody says one. */
export function lastingFor(seconds: number, t: Wording): string {
  return seconds < 60
    ? t("activity.seconds", { count: Math.max(0, Math.round(seconds)) })
    : howLong(Math.round(seconds / 60), t);
}

/** Which film or episode, as a line names it. */
function titleOf(details: Record<string, unknown>, t: Wording): string {
  const title = text(details, "title") ?? t("activity.a_title_gone");
  const series = text(details, "series");
  if (series === null) {
    return title;
  }
  const season = figure(details, "season");
  const episode = figure(details, "episode");
  const which =
    season !== null && episode !== null
      ? ` · ${t("home.up_next.short", { season, episode })}`
      : "";
  return `${series}${which} · ${title}`;
}

function joined(parts: (string | null)[]): string | null {
  const kept = parts.filter((part): part is string => part !== null && part !== "");
  return kept.length > 0 ? kept.join(" · ") : null;
}

export function sayLine(line: ActivityLine, t: Wording): Said {
  const { details } = line;
  const user = text(details, "user_name") ?? t("activity.someone");
  const device = line.device ? deviceName(line.device, t, text(details, "browser")) : null;

  switch (line.kind) {
    case "server_started":
      return {
        title: t("activity.server_started", { version: releaseOf(text(details, "version") ?? "") }),
        note: null,
      };
    case "server_stopped":
      return { title: t("activity.server_stopped"), note: null };
    case "signed_in":
    case "sign_in_refused":
    case "sign_in_held_back":
    case "signed_out":
      return { title: t(`activity.${line.kind}`, { user }), note: device };
    case "password_changed":
    case "account_created":
    case "account_removed":
      return { title: t(`activity.${line.kind}`, { user }), note: null };
    case "rights_changed":
    case "signed_out_everywhere":
      return {
        title: t(`activity.${line.kind}`, { user }),
        note: t("activity.by", { name: text(details, "by") ?? t("activity.someone") }),
      };
    case "device_signed_out":
      return {
        title: t("activity.device_signed_out", { user }),
        note: joined([device, t("activity.by", { name: text(details, "by") ?? t("activity.someone") })]),
      };
    case "account_renamed":
      return {
        title: t("activity.account_renamed", {
          previous: text(details, "previous_name") ?? t("activity.someone"),
          user,
        }),
        note: null,
      };
    case "watched": {
      const method = text(details, "method");
      const played = figure(details, "played_seconds");
      return {
        title: t("activity.watched", { user, title: titleOf(details, t) }),
        note: joined([
          played === null ? null : t("activity.played", { time: lastingFor(played, t) }),
          method === null ? null : t(`method.${method}`),
          device,
          details.stopped_by_administrator === true ? t("activity.stopped_by_administrator") : null,
        ]),
      };
    }
    case "task_finished":
    case "task_failed":
    case "task_stopped": {
      const task = text(details, "task");
      const on = (details.on ?? null) as Record<string, unknown> | null;
      const subject = on === null ? null : (text(on, "library") ?? text(on, "title"));
      const took = figure(details, "took_seconds");
      return {
        title: t(`activity.${line.kind}`, {
          task: joined([task === null ? null : t(`jobs.${task}`), subject]) ?? "",
        }),
        note:
          line.kind === "task_failed"
            ? text(details, "reason")
            : took === null
              ? null
              : t("activity.took", { time: lastingFor(took, t) }),
      };
    }
    case "works_deleted": {
      const titles = Array.isArray(details.titles)
        ? details.titles.filter((title): title is string => typeof title === "string")
        : [];
      const works = figure(details, "works") ?? titles.length;
      return {
        title: t("activity.works_deleted", { user, titles: titles.join(", ") }),
        note: joined([
          howMany(works, "activity.works", t),
          t(details.from_the_disk === true ? "activity.from_the_disk" : "activity.kept_on_the_disk"),
        ]),
      };
    }
    default:
      return { title: line.kind, note: null };
  }
}

/** When a line happened: the time today, "yesterday" and the time, or the
 *  day and the time further back. */
export function whenItHappened(at: string, now: Date, language: string, t: Wording): string {
  const instant = new Date(at);
  const time = instant.toLocaleTimeString(language, { hour: "2-digit", minute: "2-digit" });
  const dayOf = (moment: Date) =>
    new Date(moment.getFullYear(), moment.getMonth(), moment.getDate()).getTime();
  const daysAgo = Math.round((dayOf(now) - dayOf(instant)) / 86_400_000);
  if (daysAgo <= 0) {
    return time;
  }
  if (daysAgo === 1) {
    return t("activity.yesterday", { time });
  }
  const day = instant.toLocaleDateString(language, {
    day: "numeric",
    month: "short",
    year: instant.getFullYear() === now.getFullYear() ? undefined : "numeric",
  });
  return `${day} ${time}`;
}

/** One thing to look at in words, and where it is put right. */
export interface PointSaid {
  title: string;
  to: string;
}

/** Where each light of the summary is put right. */
const WORRY_LEADS_TO: Record<string, string> = {
  folder_missing: "/admin/libraries",
  disk_nearly_full: "/admin",
  card_unreachable: "/admin/transcoding",
  media_tools_missing: "/admin/diagnostics",
  database_mode: "/admin/diagnostics",
  database_refuses_writes: "/admin/diagnostics",
};

/** A light of the summary that failed its check, said as what to go and
 *  look at. */
export function sayWorry(
  worry: { kind?: string; label?: string; mount?: string; used?: number },
  t: Wording,
  language: string,
): string {
  switch (worry.kind) {
    case "folder_missing":
      return t("admin.worry.folder_missing", { label: worry.label ?? "" });
    case "disk_nearly_full":
      return t("admin.worry.disk_nearly_full", {
        mount: worry.mount ?? "",
        share: percentOf(worry.used ?? 0, language),
      });
    default:
      return t(`admin.worry.${worry.kind}`);
  }
}

export function sayPoint(point: AttentionPoint, t: Wording, language: string): PointSaid {
  const count = point.count ?? 0;
  switch (point.point) {
    case "worry":
      return {
        title: sayWorry(point, t, language),
        to: WORRY_LEADS_TO[point.kind ?? ""] ?? "/admin/diagnostics",
      };
    case "refused_sign_ins":
      return { title: howMany(count, "attention.refused_sign_ins", t), to: "/admin/journal?families=access" };
    case "failed_tasks":
      return { title: howMany(count, "attention.failed_tasks", t), to: "/admin/journal?families=library" };
    case "falling_behind":
      return { title: howMany(count, "attention.falling_behind", t), to: "/admin/playback" };
    case "unidentified":
      return { title: howMany(count, "attention.unidentified", t), to: "/admin/metadata" };
  }
}
