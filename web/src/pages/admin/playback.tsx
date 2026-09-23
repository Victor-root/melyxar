/*
 * Who is watching what, and how it reaches them.
 *
 * Every film playing on every device, played as it is or rebuilt, with what
 * was decided for it and why, how hard the machine is working on it, and a
 * way to stop it. Read again every two seconds: a film paused or left on
 * another screen shows here on its own.
 */

import { useState } from "react";
import { api } from "../../api";
import type { Watched, WatchedDecision } from "../../api";
import { refusalOf } from "../../asking";
import { PageHead, Panel, Stat } from "../../components/panel";
import { useToast } from "../../components/toasts";
import { deviceName } from "../../devices";
import { refusalKey } from "../../i18n";
import {
  DeviceIcon,
  FilmIcon,
  GraphicsCardIcon,
  HistoryIcon,
  PeopleIcon,
  PlaybackIcon,
  PlayIcon,
  SeriesIcon,
} from "../../icons";
import { asClock } from "../../player/clock";
import {
  asRate,
  asSize,
  asWork,
  KEEPING_UP,
  pictureDone,
  pictureHeld,
  reasonsSaid,
  soundHeld,
} from "../../player/describe";
import { PauseIcon } from "../../player/icons";
import { containerName } from "../../readable";
import { useSettings } from "../../settings";
import { Ghosts } from "./ghosts";
import { countedOf, episodeOf, shareWatched, useNowPlaying } from "./playing";

type Wording = (key: string, values?: Record<string, string | number>) => string;

/** What becomes of the soundtrack, in the words the player's own panel uses. */
const SOUND_DONE: Record<WatchedDecision["sound"], string> = {
  copy: "facts.carried_over",
  transcode: "facts.rebuilt_sound",
  drop: "admin.sound_dropped",
};

export function AdminPlayback() {
  const { t } = useSettings();
  const playing = useNowPlaying();
  const watched = playing.answer ?? [];

  return (
    <>
      <PageHead lead={t("admin.playback_lead")} />
      <Panel icon={PlaybackIcon} title={t("admin.playing")} lead={t("admin.playing_lead")}>
        <PlayingStats watched={playing.answer} />
        {playing.failure ? (
          <p className="empty-line">{t(refusalKey(refusalOf(playing.failure)))}</p>
        ) : watched.length === 0 ? (
          <p className="empty-line">{playing.answer && t("admin.playing_none")}</p>
        ) : (
          <div className="watches">
            {watched.map((one) => (
              <WatchCard key={one.device} watched={one} onStopAsked={playing.look} />
            ))}
          </div>
        )}
      </Panel>
      <Panel icon={HistoryIcon} title={t("admin.history")} lead={t("admin.history_lead")} soon>
        <Ghosts heads={["admin.col.when", "admin.col.who", "admin.col.work", "admin.col.way"]} />
      </Panel>
    </>
  );
}

/** The three figures of what is playing. A dash until the first answer. */
export function PlayingStats({ watched }: { watched: Watched[] | null }) {
  const { t, language } = useSettings();
  const counted = watched && countedOf(watched);
  const count = (value: number | undefined) =>
    value === undefined ? "–" : value.toLocaleString(language);

  return (
    <div className="stats stats-three">
      <Stat icon={PlaybackIcon} label={t("admin.playing_count")} value={count(counted?.playing)} />
      <Stat
        icon={GraphicsCardIcon}
        label={t("admin.transcoding_count")}
        value={count(counted?.rebuilt)}
      />
      <Stat icon={DeviceIcon} label={t("admin.direct_count")} value={count(counted?.direct)} />
    </div>
  );
}

/** How the film reaches its viewer, in a word. */
export function MethodPill({ decision }: { decision: WatchedDecision | null }) {
  const { t } = useSettings();
  if (!decision) {
    return <span className="method-pill">{t("method.unknown")}</span>;
  }
  return (
    <span className={`method-pill ${decision.expensive ? "method-rebuilt" : "method-direct"}`}>
      {t(`method.${decision.method}`)}
    </span>
  );
}

/** What is playing, how far it has got, and whether it is moving. */
export function WatchedWords({ watched }: { watched: Watched }) {
  const { t } = useSettings();
  const over = episodeOf(watched, t) ?? (watched.year !== null ? String(watched.year) : null);
  return (
    <span className="watch-words">
      {over && <span className="watch-over">{over}</span>}
      <span className="watch-title">{watched.title}</span>
      <span className="watch-who">
        <PeopleIcon size={14} />
        <span>{watched.user}</span>
        <DeviceIcon size={14} />
        <span title={watched.device_name}>{deviceName(watched.device_name, t)}</span>
      </span>
    </span>
  );
}

function WatchCard({ watched, onStopAsked }: { watched: Watched; onStopAsked: () => void }) {
  const { t } = useSettings();
  const toast = useToast();
  const [open, setOpen] = useState(false);
  const [asking, setAsking] = useState(false);
  const [sending, setSending] = useState(false);
  const share = shareWatched(watched);
  const { decision, producing } = watched;
  const Placeholder = watched.series === null ? FilmIcon : SeriesIcon;

  const stop = async () => {
    setSending(true);
    try {
      await api.stopPlaying(watched.device);
      toast({
        state: "ok",
        title: t("admin.stop_asked"),
        detail: t("admin.stop_asked_why", { user: watched.user }),
      });
    } catch (error) {
      toast({
        state: refusalOf(error) === "not_found" ? "attention" : "trouble",
        title: t(refusalOf(error) === "not_found" ? "admin.stop_already" : "admin.stop_failed"),
        detail:
          refusalOf(error) === "not_found" ? undefined : t(refusalKey(refusalOf(error))),
      });
    } finally {
      setSending(false);
      setAsking(false);
      onStopAsked();
    }
  };

  return (
    <article className={`watch${watched.stopping ? " watch-stopping" : ""}`}>
      <div className="watch-main">
        <div className="watch-picture">
          {watched.picture ? (
            <img src={watched.picture} alt="" loading="lazy" />
          ) : (
            <Placeholder size={30} />
          )}
          <span className={`watch-moving${watched.paused ? "" : " watch-moving-on"}`}>
            {watched.paused ? <PauseIcon size={14} /> : <PlayIcon size={14} />}
            {t(watched.paused ? "admin.paused" : "admin.playing_now")}
          </span>
        </div>

        <div className="watch-body">
          <WatchedWords watched={watched} />

          <div className="watch-progress">
            <span className="meter">
              <span
                className="meter-fill"
                style={{ width: `${share === null ? 0 : share * 100}%` }}
              />
            </span>
            <span className="watch-time">
              {asClock(watched.position_seconds)}
              {watched.duration_seconds !== null && ` / ${asClock(watched.duration_seconds)}`}
            </span>
          </div>

          {decision?.expensive && (
            <p className="watch-why">{reasonsSaid(decision.reasons, t)}</p>
          )}

          <div className="watch-foot">
            <MethodPill decision={decision} />
            {producing && (
              <span
                className={`watch-speed${producing.speed < KEEPING_UP ? " watch-speed-behind" : ""}`}
              >
                <GraphicsCardIcon size={15} />
                {decision?.rebuild && t(`admin.rebuilt_by.${decision.rebuild.by}`)}
                {" · "}
                {asWork(producing, t)}
              </span>
            )}
            {watched.stopping && <span className="watch-stop-word">{t("admin.stopping")}</span>}
            <span className="watch-actions">
              <button
                className="button button-small button-quiet"
                aria-expanded={open}
                onClick={() => setOpen((was) => !was)}
              >
                {t(open ? "admin.less" : "admin.details")}
              </button>
              {!watched.stopping &&
                (asking ? (
                  <>
                    <button
                      className="button button-small button-quiet"
                      onClick={() => setAsking(false)}
                      disabled={sending}
                    >
                      {t("admin.cancel")}
                    </button>
                    <button
                      className="button button-small button-danger-full"
                      onClick={stop}
                      disabled={sending}
                    >
                      {t("admin.stop_confirm")}
                    </button>
                  </>
                ) : (
                  <button className="button button-small button-danger" onClick={() => setAsking(true)}>
                    {t("admin.stop")}
                  </button>
                ))}
            </span>
          </div>
        </div>
      </div>

      {open && <WatchDetails watched={watched} t={t} />}
    </article>
  );
}

/** One fact of the details: a name and what it is, or nothing at all. */
function Fact({ name, is, behind = false }: { name: string; is: string | null; behind?: boolean }) {
  if (is === null || is === "") {
    return null;
  }
  return (
    <>
      <dt>{name}</dt>
      <dd className={behind ? "watch-behind" : undefined}>{is}</dd>
    </>
  );
}

/** What the file holds beside what is made of it, and who it is made for. */
function WatchDetails({ watched, t }: { watched: Watched; t: Wording }) {
  const { language } = useSettings();
  const { decision, producing } = watched;
  const picture = decision?.film.picture ?? null;
  const sound = decision?.film.sound ?? null;

  return (
    <div className="watch-details">
      {decision && (
        <section>
          <h4>{t("facts.stream")}</h4>
          <dl>
            <Fact name={t("facts.method")} is={t(`playback.${decision.method}`)} />
            <Fact name={t("facts.why")} is={reasonsSaid(decision.reasons, t)} />
            <Fact
              name={t("facts.working")}
              is={producing ? asWork(producing, t) : null}
              behind={producing !== null && producing.speed < KEEPING_UP}
            />
          </dl>
        </section>
      )}

      {decision && picture && (
        <section>
          <h4>{t("facts.picture")}</h4>
          <dl>
            <Fact name={t("facts.held")} is={pictureHeld(picture, t)} />
            <Fact name={t("facts.done")} is={pictureDone(decision.rebuild, t)} />
            <Fact
              name={t("admin.card")}
              is={
                decision.card_way
                  ? `${decision.card_way.toUpperCase()} · ${t(
                      decision.card_reads_the_film ? "admin.card_reads" : "admin.card_writes",
                    )}`
                  : null
              }
            />
            <Fact name={t("admin.tone_map")} is={decision.tone_map ? t("admin.tone_map_on") : null} />
          </dl>
        </section>
      )}

      {decision && (
        <section>
          <h4>{t("facts.sound")}</h4>
          <dl>
            <Fact name={t("facts.held")} is={sound && soundHeld(sound, t)} />
            <Fact name={t("facts.done")} is={t(SOUND_DONE[decision.sound])} />
            <Fact name={t("admin.subtitles")} is={t(`admin.subtitles.${decision.subtitles}`)} />
          </dl>
        </section>
      )}

      {decision && (
        <section>
          <h4>{t("facts.file")}</h4>
          <dl>
            <Fact
              name={t("facts.container")}
              is={decision.film.container && containerName(decision.film.container)}
            />
            <Fact name={t("facts.size")} is={asSize(decision.film.size_bytes)} />
            <Fact name={t("facts.rate")} is={asRate(decision.film.overall_bitrate)} />
          </dl>
        </section>
      )}

      <section className="watch-details-across">
        <h4>{t("admin.col.device")}</h4>
        <dl>
          <Fact
            name={t("admin.started_at")}
            is={new Date(watched.started_at).toLocaleTimeString(language, {
              hour: "2-digit",
              minute: "2-digit",
            })}
          />
          <Fact name={t("admin.device_said")} is={watched.device_name} />
        </dl>
      </section>
    </div>
  );
}
