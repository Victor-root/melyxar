/*
 * The player.
 *
 * A film the browser opens by itself is handed to a plain video element: the
 * browser already knows how to fetch the stretch it needs, draw the frames and
 * let someone scrub, and nothing here improves on that.
 *
 * A film it cannot open is rebuilt by the server as it is watched, and arrives
 * cut into segments listed in a playlist. Only Apple's browsers read that on
 * their own, so everywhere else a library feeds the segments in. That is the
 * only reason it is here.
 *
 * The position is reported while watching, so closing the tab in the middle of
 * a film does not lose it. It is reported on a timer rather than on every
 * frame, and once more when the tab goes away, which is the moment that
 * actually matters.
 */

import type Hls from "hls.js";
import { useCallback, useEffect, useRef, useState } from "react";
import { api, ApiError } from "../api";
import type { PlaybackPlan, PlaybackSession, PlaybackTrack } from "../api";
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
 * How long a segment may take to arrive before the library gives up on it.
 *
 * Set above the patience the server itself has: a segment being rebuilt right
 * now is worth waiting for, and asking again would only start the same work
 * twice.
 */
const SEGMENT_PATIENCE = 40_000;

/** Whether the film can reach this browser without being rebuilt. */
function canBePlayedAsItIs(plan: PlaybackPlan): boolean {
  return plan.method === "direct_play";
}

/** What went wrong, in a word this interface knows how to say. */
function wording(error: unknown): string {
  if (!(error instanceof ApiError)) {
    return "error.unreachable";
  }
  switch (error.code) {
    case "root_unavailable":
      return "player.missing";
    case "conflict":
      return "player.too_busy";
    case "dependency_missing":
      return "player.no_conversion";
    default:
      return "error.unreachable";
  }
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
  const [stream, setStream] = useState<PlaybackSession | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [audioId, setAudioId] = useState<string | null>(null);
  const [subtitleId, setSubtitleId] = useState<string | null>(null);
  const [speed, setSpeed] = useState(1);
  /* The last position seen, kept apart from the element. On the way out the
     element is already gone, and that is exactly the moment the position is
     worth sending. */
  const lastPosition = useRef(0);
  /* Where the picture picks up once it is ready, and null when it starts
     where it is. Applied on loadedmetadata: set any earlier it is ignored
     without a word, and the film starts from the beginning. */
  const resumeAt = useRef<number | null>(null);
  /* Whether the first answer has been seen. The resume point comes from that
     one and no other: later answers carry the same number, and applying it
     again would drag a viewer back to where they were an hour ago. */
  const opened = useRef(false);
  /* The session being watched, kept apart from the element for the same
     reason as the position: on the way out there is nothing left to read it
     from, and that is exactly when it has to be closed. */
  const session = useRef<string | null>(null);

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
        if (!opened.current) {
          opened.current = true;
          resumeAt.current = fromTheStart ? null : answer.resume_from_seconds;
        }
        setPlan(answer);
        setFailed(null);
      })
      .catch((error) => {
        if (!(error instanceof DOMException)) {
          setFailed(wording(error));
        }
      });
    return () => controller.abort();
  }, [sourceId, audioId, subtitleId, fromTheStart]);

  const rebuilt = plan !== null && !canBePlayedAsItIs(plan);

  /* A film this browser cannot open is rebuilt by the server, which needs a
     session to rebuild it into. The session is closed on the way out, and by
     the server itself if that never arrives: a viewer who closes a tab says
     nothing, and a tool left running is a core spent on nobody. */
  useEffect(() => {
    if (!rebuilt) {
      setStream(null);
      return;
    }

    /* Picking another soundtrack starts a new session from nothing, and its
       picture would open at the beginning. Carrying the position over is what
       makes changing a track feel like changing a track. */
    if (lastPosition.current > 0) {
      resumeAt.current = lastPosition.current;
    }

    const controller = new AbortController();
    let gone = false;

    api
      .openSession(
        sourceId,
        {
          profile: clientProfile(),
          audio_track_id: audioId,
          subtitle_track_id: subtitleId,
        },
        controller.signal,
      )
      .then((opening) => {
        if (gone) {
          api.closeSession(opening.id);
          return;
        }
        session.current = opening.id;
        setStream(opening);
      })
      .catch((error) => {
        if (!(error instanceof DOMException)) {
          setFailed(wording(error));
        }
      });

    return () => {
      gone = true;
      controller.abort();
      if (session.current) {
        api.closeSession(session.current);
        session.current = null;
      }
    };
  }, [rebuilt, sourceId, audioId, subtitleId]);

  /* Feeding the segments in. Apple's browsers read a playlist on their own,
     so there the address goes straight to the element and nothing else is
     needed. */
  useEffect(() => {
    const element = video.current;
    if (!element || !stream) {
      return;
    }

    if (element.canPlayType("application/vnd.apple.mpegurl")) {
      element.src = stream.playlist_url;
      return;
    }

    let feed: Hls | null = null;
    let gone = false;

    /* Fetched only now, and only by the viewer who needs it. Most films play
       as they are and never touch this, so carrying it in every page would
       slow the whole interface for the few that do. */
    void import("hls.js").then(({ default: Library }) => {
      if (gone) {
        return;
      }
      if (!Library.isSupported()) {
        setFailed("player.cannot_play");
        return;
      }
      feed = new Library({ fragLoadingTimeOut: SEGMENT_PATIENCE });
      feed.on(Library.Events.ERROR, (_event, trouble) => {
        // Anything short of fatal is retried on its own, and saying so would
        // turn an invisible hiccup into an error the viewer has to read.
        if (trouble.fatal) {
          setFailed("player.cannot_play");
        }
      });
      feed.loadSource(stream.playlist_url);
      feed.attachMedia(element);
    });

    return () => {
      gone = true;
      feed?.destroy();
    };
  }, [stream]);

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
     usually dropped. Both of these are handed to the browser to deliver on its
     own behalf, which is what that is for.

     The session goes with the position: React tears nothing down when a tab
     closes, so without this the server would keep converting a film nobody is
     watching until it notices on its own, two minutes later. */
  const onTheWayOut = useCallback(() => {
    const seconds = lastPosition.current;
    if (seconds >= WORTH_REPORTING) {
      api.reportPositionOnTheWayOut(workId, seconds);
    }
    if (session.current) {
      api.closeSession(session.current);
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
    window.addEventListener("pagehide", onTheWayOut);
    return () => {
      window.clearInterval(timer);
      document.removeEventListener("visibilitychange", onHidden);
      window.removeEventListener("pagehide", onTheWayOut);
      report();
    };
  }, [report, onTheWayOut]);

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
    if (!element) {
      return;
    }
    element.playbackRate = speed;
    const target = resumeAt.current;
    resumeAt.current = null;
    if (target !== null && target > 0) {
      element.currentTime = target;
    }
  };

  return (
    <div className="player" role="dialog" aria-label={title}>
      <div className="player-bar">
        <button className="button" onClick={onClose}>
          {t("player.close")}
        </button>
        <span className="player-title">{title}</span>
        {rebuilt && <span className="fact">{t("player.rebuilt")}</span>}
      </div>

      {failed && <p className="notice">{t(failed)}</p>}

      {/* The session is asked for and the first segments are produced, which
          takes a moment on a film nobody has played yet. */}
      {rebuilt && !stream && !failed && <p className="notice">{t("player.preparing")}</p>}

      {/* One element for both, told apart by its key: a film handed over as a
          file carries its address, a rebuilt one is fed by the library, and
          switching between the two has to start from a fresh element rather
          than from one still holding the other's address. */}
      {plan && !failed && (canBePlayedAsItIs(plan) || stream) && (
        <video
          key={canBePlayedAsItIs(plan) ? "file" : stream?.id}
          ref={video}
          className="player-video"
          src={canBePlayedAsItIs(plan) ? plan.url : undefined}
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
