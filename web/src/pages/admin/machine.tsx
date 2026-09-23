/*
 * What the machine is spending: live figures, and how each moved over the
 * stretch of time chosen.
 *
 * The figures are read every two seconds while the page is open, and only
 * then; the curves are read again once a minute, which is how often a point
 * is added to them. The last ten minutes come from the server's memory at
 * the beat of the figures themselves, which is what makes the shortest curve
 * move as it is watched.
 */

import { useEffect, useState } from "react";
import type { ReactNode } from "react";
import { api } from "../../api";
import type { LiveMeasures, MeasuredDisk, MeasuredOver, MeasurePoint } from "../../api";
import { useAsked } from "../../asking";
import { Panel, Picker } from "../../components/panel";
import { Sparkline } from "../../components/sparkline";
import {
  ChipIcon,
  ClockIcon,
  DiskIcon,
  GraphicsCardIcon,
  MemoryIcon,
  NetworkIcon,
} from "../../icons";
import type { IconProps } from "../../icons";
import { amountOfData, networkRate, percentOf } from "../../readable";
import { safeRead, safeWrite } from "../../i18n";
import { useSettings } from "../../settings";
import { diskName, fullness, usedShare } from "./disks";

/** How often the live figures are read. */
const LIVE_EVERY_MS = 2_000;

/** How often the curves are read again: a point is added once a minute. */
const CURVES_EVERY_MS = 60_000;

/** How far back the curves reach: the last ten minutes, kept in the server's
 *  memory, or a stretch kept in its database. */
type Reach = "live" | MeasuredOver;

const REACHES: Reach[] = ["live", "hour", "day", "week", "month", "year"];

/** Kept by this browser: the stretch somebody likes to look at is theirs. */
const STORED_REACH = "melyxar.admin.reach";

function initialReach(): Reach {
  const stored = safeRead(STORED_REACH);
  return REACHES.find((reach) => reach === stored) ?? "live";
}

export function SystemPanel({ card }: { card: string | null }) {
  const { t } = useSettings();
  const [reach, setReach] = useState<Reach>(initialReach);

  const live = useAsked((signal) => api.measures(signal));
  const lookLive = live.look;
  useEffect(() => {
    const timer = window.setInterval(lookLive, LIVE_EVERY_MS);
    return () => window.clearInterval(timer);
  }, [lookLive]);

  const kept = useAsked(
    (signal) => (reach === "live" ? Promise.resolve([]) : api.measuresOver(reach, signal)),
    [reach],
  );
  const lookKept = kept.look;
  useEffect(() => {
    const timer = window.setInterval(lookKept, CURVES_EVERY_MS);
    return () => window.clearInterval(timer);
  }, [lookKept]);

  const choose = (picked: Reach) => {
    safeWrite(STORED_REACH, picked);
    setReach(picked);
  };

  const points = reach === "live" ? (live.answer?.recent ?? []) : (kept.answer ?? []);

  return (
    <Panel
      icon={ChipIcon}
      title={t("admin.system")}
      lead={t("admin.system_lead")}
      className="panel-system"
      action={
        <Picker<Reach>
          label={t("admin.reach")}
          value={reach}
          options={REACHES.map((one) => [one, t(`admin.reach.${one}`)] as const)}
          onPick={choose}
        />
      }
    >
      {live.failure && !live.answer ? (
        <p className="panel-notice panel-notice-trouble">{t("admin.measures_unreachable")}</p>
      ) : (
        <>
          <Gauges live={live.answer} points={points} reach={reach} card={card} />
          {live.answer && live.answer.disks.length > 0 && <Disks disks={live.answer.disks} />}
        </>
      )}
    </Panel>
  );
}

/** Every disk the server uses, each by its name and what it holds. */
function Disks({ disks }: { disks: MeasuredDisk[] }) {
  const { t, language } = useSettings();
  return (
    <section className="disks">
      <h3 className="disks-title">{t("admin.disks")}</h3>
      <div className="lines">
        {disks.map((disk) => {
          const used = usedShare(disk);
          const state = fullness(used);
          const holds = [...disk.libraries, ...(disk.holds_the_server ? [t("admin.disk_server")] : [])];
          return (
            <div key={disk.mount} className="line disk-line" title={disk.mount}>
              <span className={`line-mark${state ? ` state-${state}` : ""}`} aria-hidden="true">
                <DiskIcon size={18} />
              </span>
              <span className="line-words">
                <span className="line-name">{diskName(disk.mount) ?? t("admin.disk_system")}</span>
                <span className="line-note">{holds.join(" · ")}</span>
              </span>
              <span className="disk-line-meter">
                <Meter used={used} />
              </span>
              <span className="disk-line-figures">
                <span className={`line-figure${state ? ` state-${state}` : ""}`}>{percentOf(used, language)}</span>
                <span className="line-note">
                  {t("admin.disk_free", {
                    free: amountOfData(disk.available_bytes, language),
                    total: amountOfData(disk.total_bytes, language),
                  })}
                </span>
              </span>
            </div>
          );
        })}
      </div>
    </section>
  );
}

/** How full something is, in the colour of its state once it runs short. */
function Meter({ used }: { used: number }) {
  const state = fullness(used);
  return (
    <span className="meter">
      <span
        className={`meter-fill${state ? ` meter-fill-${state}` : ""}`}
        style={{ width: `${Math.round(used * 100)}%` }}
      />
    </span>
  );
}

function Gauges({
  live,
  points,
  reach,
  card,
}: {
  live: LiveMeasures | null;
  points: MeasurePoint[];
  reach: Reach;
  card: string | null;
}) {
  const { t, language } = useSettings();
  const now = live?.recent[live.recent.length - 1] ?? null;
  const when = (index: number) => whenOf(points[index]?.at, reach, language);
  const share = (value: number | null | undefined) =>
    value === null || value === undefined ? "–" : percentOf(value, language);

  const memoryShare = (point: MeasurePoint) =>
    point.memory_total > 0 ? point.memory_used / point.memory_total : null;

  const disks = live?.disks ?? [];
  const storageTotal = disks.reduce((sum, disk) => sum + disk.total_bytes, 0);
  const storageUsed = disks.reduce((sum, disk) => sum + disk.total_bytes - disk.available_bytes, 0);

  return (
    <div className="gauges">
      <Gauge
        icon={ChipIcon}
        label={t("admin.cpu")}
        value={share(now?.processor)}
        note={live?.machine.processor ?? undefined}
        curve={
          <Sparkline
            label={t("admin.cpu")}
            ceiling={1}
            series={[{ values: points.map((point) => point.processor) }]}
            say={(index) => `${share(points[index]?.processor)} · ${when(index)}`}
          />
        }
      />

      <Gauge
        icon={MemoryIcon}
        label={t("admin.memory")}
        value={
          now ? `${amountOfData(now.memory_used, language)} / ${amountOfData(now.memory_total, language)}` : "–"
        }
        note={now && now.memory_total > 0 ? share(memoryShare(now)) : undefined}
        curve={
          <Sparkline
            label={t("admin.memory")}
            ceiling={1}
            series={[{ values: points.map(memoryShare) }]}
            say={(index) =>
              `${points[index] ? amountOfData(points[index].memory_used, language) : "–"} · ${when(index)}`
            }
          />
        }
      />

      <Gauge
        icon={DiskIcon}
        label={t("admin.storage")}
        value={
          storageTotal > 0
            ? `${amountOfData(storageUsed, language)} / ${amountOfData(storageTotal, language)}`
            : "–"
        }
        note={
          storageTotal > 0
            ? t(disks.length === 1 ? "admin.storage_note_one" : "admin.storage_note", {
                share: share(storageUsed / storageTotal),
                disks: disks.length,
              })
            : undefined
        }
        curve={
          storageTotal > 0 ? (
            <span className="gauge-meter">
              <Meter used={storageUsed / storageTotal} />
            </span>
          ) : undefined
        }
      />

      <Gauge
        icon={NetworkIcon}
        label={t("admin.network")}
        value={
          now ? (
            <span className="network-figures">
              <span>
                <span className="network-way">↑</span>
                {networkRate(now.sent, language)}
              </span>
              <span className="network-in">
                <span className="network-way">↓</span>
                {networkRate(now.received, language)}
              </span>
            </span>
          ) : (
            "–"
          )
        }
        note={t("admin.network_note")}
        curve={
          <Sparkline
            label={t("admin.network")}
            series={[
              { values: points.map((point) => point.sent) },
              { values: points.map((point) => point.received), quiet: true },
            ]}
            say={(index) =>
              points[index]
                ? `↑ ${networkRate(points[index].sent, language)} ↓ ${networkRate(points[index].received, language)} · ${when(index)}`
                : ""
            }
          />
        }
      />

      <Gauge
        icon={GraphicsCardIcon}
        label={t("admin.card")}
        value={card ? share(now?.card) : t("admin.card_unused")}
        note={card ? card.toUpperCase() : t("admin.card_unused_why")}
        curve={
          card ? (
            <Sparkline
              label={t("admin.card")}
              ceiling={1}
              series={[{ values: points.map((point) => point.card) }]}
              say={(index) => `${share(points[index]?.card)} · ${when(index)}`}
            />
          ) : undefined
        }
      />

      <Gauge
        icon={ClockIcon}
        label={t("admin.load")}
        value={
          now?.load === null || now?.load === undefined
            ? "–"
            : now.load.toLocaleString(language, { maximumFractionDigits: 2, minimumFractionDigits: 2 })
        }
        note={
          [
            live ? t("admin.load_of", { threads: live.machine.threads }) : null,
            now?.temperature !== null && now?.temperature !== undefined
              ? t("admin.temperature", {
                  degrees: now.temperature.toLocaleString(language, { maximumFractionDigits: 0 }),
                })
              : null,
          ]
            .filter(Boolean)
            .join(" · ") || undefined
        }
        curve={
          <Sparkline
            label={t("admin.load")}
            series={[{ values: points.map((point) => point.load) }]}
            say={(index) =>
              `${points[index]?.load?.toLocaleString(language, { maximumFractionDigits: 2 }) ?? "–"} · ${when(index)}`
            }
          />
        }
      />
    </div>
  );
}

/** One figure: what it is, what it is now, and how it moved. */
function Gauge({
  icon: GaugeIcon,
  label,
  value,
  note,
  curve,
}: {
  icon: (props: IconProps) => ReactNode;
  label: string;
  value: ReactNode;
  note?: string;
  curve?: ReactNode;
}) {
  return (
    <div className="gauge">
      <div className="gauge-head">
        <span className="stat-mark" aria-hidden="true">
          <GaugeIcon size={20} />
        </span>
        <span className="stat-words">
          <span className="stat-label">{label}</span>
          <span className="stat-value">{value}</span>
          {note && (
            <span className="stat-note" title={note}>
              {note}
            </span>
          )}
        </span>
      </div>
      {curve}
    </div>
  );
}

/** When a point was, as precisely as the stretch it belongs to asks for. */
function whenOf(at: string | undefined, reach: Reach, language: string): string {
  if (!at) {
    return "";
  }
  const instant = new Date(at);
  switch (reach) {
    case "live":
      return instant.toLocaleTimeString(language, { hour: "2-digit", minute: "2-digit", second: "2-digit" });
    case "hour":
    case "day":
      return instant.toLocaleTimeString(language, { hour: "2-digit", minute: "2-digit" });
    case "week":
    case "month":
      return instant.toLocaleString(language, {
        weekday: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      });
    case "year":
      return instant.toLocaleDateString(language, { day: "numeric", month: "short" });
  }
}
