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
import type { PlaybackPlan } from "../api";
import { useSettings } from "../settings";
import { clientProfile } from "./profile";

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

export function Player({
  sourceId,
  workId,
  title,
  onClose,
}: {
  sourceId: string;
  workId: string;
  title: string;
  onClose: () => void;
}) {
  const { t } = useSettings();
  const video = useRef<HTMLVideoElement>(null);
  const [plan, setPlan] = useState<PlaybackPlan | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [resumed, setResumed] = useState(false);
  /* The last position seen, kept apart from the element. On the way out the
     element is already gone, and that is exactly the moment the position is
     worth sending. */
  const lastPosition = useRef(0);

  useEffect(() => {
    const controller = new AbortController();
    api
      .plan(sourceId, { profile: clientProfile() }, controller.signal)
      .then(setPlan)
      .catch((error) => {
        if (!(error instanceof DOMException)) {
          setFailed(error.code === "root_unavailable" ? "player.missing" : "error.unreachable");
        }
      });
    return () => controller.abort();
  }, [sourceId]);

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
    if (!element || resumed || !plan?.resume_from_seconds) {
      return;
    }
    element.currentTime = plan.resume_from_seconds;
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
