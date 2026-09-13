/*
 * The player.
 *
 * A plain video element rather than a library: the browser already knows how
 * to fetch the stretch it needs, draw the frames and let someone scrub. What
 * is written here is only what the browser does not do on its own, and each
 * piece is here for a reason a viewer would recognise.
 *
 * The position is reported while watching, so closing the tab in the middle of
 * a film does not lose it. It is reported on a timer rather than on every
 * frame, and once more when the tab goes away, which is the moment that
 * actually matters.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../api";
import type { PlaybackPlan, PlaybackTrack } from "../api";
import { useSettings } from "../settings";
import { languageName } from "./languages";
import { clientProfile } from "./profile";

/** The speeds offered. Whole steps: nobody asks for 1.17 times. */
const SPEEDS = [0.75, 1, 1.25, 1.5, 1.75, 2];

/** How often the position is sent while a film plays. */
const REPORT_EVERY = 10_000;

/** Below this, a film counts as not started, so leaving at once loses nothing. */
const WORTH_REPORTING = 5;

/**
 * Whether the film can reach this browser without being rebuilt.
 *
 * Rebuilding one is the next milestone. Until it exists, a film that needs it
 * is announced as such rather than handed to a player that shows nothing.
 */
function canBePlayedAsItIs(plan: PlaybackPlan): boolean {
  return plan.method === "direct_play";
}

/**
 * What to call a track in a picker.
 *
 * The language first, since that is what a viewer is looking for, then what
 * the file itself calls it when it says something, and the number of channels
 * when there is more than a pair.
 */
function trackName(
  track: PlaybackTrack,
  t: (key: string) => string,
  speaking: string,
): string {
  const parts = [
    track.language ? languageName(track.language, speaking) : t("player.unknown_language"),
  ];
  if (track.title) {
    parts.push(track.title);
  }
  if (track.channels && track.channels > 2) {
    parts.push(`${track.channels}`);
  }
  if (track.burns_in) {
    parts.push(t("player.burns_in_short"));
  }
  return parts.join(" · ");
}

export function Player({
  sourceId,
  workId,
  title,
  fromTheStart,
  onClose,
}: {
  sourceId: string;
  workId: string;
  title: string;
  /** Set when the viewer asked to start again rather than carry on. */
  fromTheStart?: boolean;
  onClose: () => void;
}) {
  const { t, language } = useSettings();
  const video = useRef<HTMLVideoElement>(null);
  const [plan, setPlan] = useState<PlaybackPlan | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [resumed, setResumed] = useState(false);
  const [audioId, setAudioId] = useState<string | null>(null);
  const [subtitleId, setSubtitleId] = useState<string | null>(null);
  const [speed, setSpeed] = useState(1);
  /* The last position seen, kept apart from the element. On the way out the
     element is already gone, and that is exactly the moment the position is
     worth sending. */
  const lastPosition = useRef(0);

  /* Asked again whenever a track changes: which tracks are wanted is part of
     the question, and the answer can change with it. A film played as it is
     becomes one that has to be rebuilt the moment someone picks a subtitle
     that can only be drawn into the picture. */
  useEffect(() => {
    const controller = new AbortController();
    api
      .plan(
        sourceId,
        {
          profile: clientProfile(),
          audio_track_id: audioId,
          subtitle_track_id: subtitleId,
        },
        controller.signal,
      )
      .then((answer) => {
        setPlan(answer);
        setFailed(null);
      })
      .catch((error) => {
        if (!(error instanceof DOMException)) {
          setFailed(error.code === "root_unavailable" ? "player.missing" : "error.unreachable");
        }
      });
    return () => controller.abort();
  }, [sourceId, audioId, subtitleId]);

  /* A choice is remembered once it has been made, never on the way in: the
     tracks a page opens with are what the rules already decided, and writing
     them back would turn a habit into a decision nobody made. */
  const choose = (audio: string | null, subtitle: string | null) => {
    setAudioId(audio);
    setSubtitleId(subtitle);
    api
      .rememberTracks({
        work_id: workId,
        source_id: sourceId,
        audio_track_id: audio,
        subtitle_track_id: subtitle,
      })
      .catch(() => {
        // A choice that could not be written down still applies to this
        // sitting, which is the part the viewer is watching.
      });
  };

  const report = useCallback(() => {
    const seconds = lastPosition.current;
    if (seconds < WORTH_REPORTING) {
      return;
    }
    api.reportPosition(workId, seconds).catch(() => {
      // A position that could not be sent is not worth troubling a viewer
      // with: the next one carries the same news, and the film keeps playing.
    });
  }, [workId]);

  /* Closing the tab is how most films are left, and a request started then is
     usually dropped. This one is handed to the browser to deliver on its own
     behalf, which is what it is for. */
  const reportOnTheWayOut = useCallback(() => {
    const seconds = lastPosition.current;
    if (seconds >= WORTH_REPORTING) {
      api.reportPositionOnTheWayOut(workId, seconds);
    }
  }, [workId]);

  // While playing, and once more when the tab goes away. The second is the
  // one that matters: closing a tab is how most films are left.
  useEffect(() => {
    const timer = window.setInterval(report, REPORT_EVERY);
    const onHidden = () => {
      if (document.visibilityState === "hidden") {
        report();
      }
    };
    document.addEventListener("visibilitychange", onHidden);
    window.addEventListener("pagehide", reportOnTheWayOut);
    return () => {
      window.clearInterval(timer);
      document.removeEventListener("visibilitychange", onHidden);
      window.removeEventListener("pagehide", reportOnTheWayOut);
      report();
    };
  }, [report, reportOnTheWayOut]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const element = video.current;
      if (!element) {
        return;
      }
      switch (event.key) {
        case "Escape":
          onClose();
          break;
        case " ":
        case "k":
          event.preventDefault();
          if (element.paused) {
            void element.play();
          } else {
            element.pause();
          }
          break;
        case "ArrowLeft":
          element.currentTime = Math.max(0, element.currentTime - 10);
          break;
        case "ArrowRight":
          element.currentTime += 10;
          break;
        case "f":
          void element.requestFullscreen?.();
          break;
        default:
          break;
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  /* Where the viewer stopped, applied once the browser knows how long the
     film is. Setting it earlier is ignored, silently, and the film starts
     from the beginning as though nothing had been watched. */
  const onReady = () => {
    const element = video.current;
    if (!element || resumed) {
      return;
    }
    element.playbackRate = speed;
    if (!fromTheStart && plan?.resume_from_seconds) {
      element.currentTime = plan.resume_from_seconds;
    }
    setResumed(true);
  };

  return (
    <div className="player" role="dialog" aria-label={title}>
      <div className="player-bar">
        <button className="button" onClick={onClose}>
          {t("player.close")}
        </button>
        <span className="player-title">{title}</span>
        {plan && !canBePlayedAsItIs(plan) && (
          <span className="fact fact-warning">{t("player.not_yet")}</span>
        )}
      </div>

      {failed && <p className="notice">{t(failed)}</p>}

      {/* Only a film this browser opens as it is can be played today.
          Converting one that it cannot is a whole engine, and it is the next
          milestone: saying so plainly beats a black rectangle. */}
      {plan && !failed && !canBePlayedAsItIs(plan) && (
        <p className="notice">{t("player.conversion_not_built")}</p>
      )}

      {plan && !failed && canBePlayedAsItIs(plan) && (
        <video
          ref={video}
          className="player-video"
          src={plan.url}
          controls
          autoPlay
          onLoadedMetadata={onReady}
          onTimeUpdate={(event) => {
            lastPosition.current = event.currentTarget.currentTime;
          }}
          onPause={report}
          onEnded={report}
          onError={() => setFailed("player.cannot_play")}
        />
      )}

      {/* Shown even when the film cannot be played as it is: choosing a track
          is what caused that, and the way back is the same picker. Hiding it
          would leave a viewer stuck with a message. */}
      {plan && !failed && (
        <div className="player-choices">
          {plan.audio.length > 1 && (
            <label className="choice">
              <span className="choice-label">{t("work.audio")}</span>
              <select
                value={plan.chosen_audio_id ?? ""}
                onChange={(event) => choose(event.target.value || null, subtitleId)}
              >
                {plan.audio.map((track) => (
                  <option key={track.id} value={track.id}>
                    {trackName(track, t, language)}
                  </option>
                ))}
              </select>
            </label>
          )}

          {plan.subtitles.length > 0 && (
            <label className="choice">
              <span className="choice-label">{t("work.subtitles")}</span>
              <select
                value={plan.chosen_subtitle_id ?? ""}
                onChange={(event) => choose(audioId, event.target.value || null)}
              >
                <option value="">{t("player.no_subtitle")}</option>
                {plan.subtitles.map((track) => (
                  <option key={track.id} value={track.id}>
                    {trackName(track, t, language)}
                  </option>
                ))}
              </select>
            </label>
          )}

          <label className="choice">
            <span className="choice-label">{t("player.speed")}</span>
            <select
              value={speed}
              onChange={(event) => {
                const chosen = Number(event.target.value);
                setSpeed(chosen);
                if (video.current) {
                  video.current.playbackRate = chosen;
                }
              }}
            >
              {SPEEDS.map((value) => (
                <option key={value} value={value}>
                  {value}&times;
                </option>
              ))}
            </select>
          </label>

          {/* The picture in a corner while the viewer does something else.
              Not every browser offers it, so the button only appears where
              it works. */}
          {"requestPictureInPicture" in HTMLVideoElement.prototype && (
            <button
              className="toggle"
              onClick={() => {
                void video.current?.requestPictureInPicture?.();
              }}
            >
              {t("player.corner")}
            </button>
          )}
        </div>
      )}

      {plan && (
        <p className="player-why">
          {t(`playback.${plan.method}`)}
          {plan.reasons.length > 0 && (
            <span className="player-reasons">
              {plan.reasons.map((reason) => t(`reason.${reason.code}`)).join(" · ")}
            </span>
          )}
        </p>
      )}
    </div>
  );
}
