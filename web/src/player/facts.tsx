/*
 * What is happening to this film, while it plays.
 *
 * A film that looks or sounds wrong is one question in two halves: what the
 * file holds, and what is being done to it. Either half alone explains
 * nothing, and until now the second half lived in one line under the picture
 * and the first half nowhere at all. Answering it meant reading the server's
 * journal, which is not where somebody watching a film is.
 *
 * Read live off the element rather than remembered: what the browser ended up
 * with is the only side of this the server cannot see, and it is the side that
 * says whether a picture arrived the shape it left.
 */

import { useEffect, useState } from "react";

import { api } from "../api";
import type { CalibrationEntry, PlaybackPlan, Producing } from "../api";
import { storedCalibration } from "./calibration";

/** How often what the browser says is read again. */
const LOOK_EVERY_MS = 1_000;

/** Below this the machine is producing the film more slowly than it plays. */
const KEEPING_UP = 1;

/** What the browser says about the picture it is showing. */
interface WhatTheBrowserSays {
  across: number;
  down: number;
  drawnAcross: number;
  drawnDown: number;
  shown: number;
  dropped: number;
  heldTo: number | null;
}

function readTheElement(element: HTMLVideoElement): WhatTheBrowserSays {
  const quality = element.getVideoPlaybackQuality?.();
  const held = element.buffered;
  let heldTo: number | null = null;
  for (let index = 0; index < held.length; index += 1) {
    if (held.start(index) <= element.currentTime && element.currentTime <= held.end(index)) {
      heldTo = held.end(index);
    }
  }
  return {
    across: element.videoWidth,
    down: element.videoHeight,
    drawnAcross: element.clientWidth,
    drawnDown: element.clientHeight,
    shown: quality ? quality.totalVideoFrames : 0,
    dropped: quality ? quality.droppedVideoFrames : 0,
    heldTo,
  };
}

/** A rate in bits per second, as somebody reads one. */
function asRate(bits: number | null): string | null {
  if (bits === null || bits <= 0) {
    return null;
  }
  return bits >= 1_000_000
    ? `${(bits / 1_000_000).toFixed(bits >= 10_000_000 ? 0 : 1)} Mb/s`
    : `${Math.round(bits / 1_000)} kb/s`;
}

/** How the tool's work reads, when it is doing any.
 *
 * Pictures a second only when the tool is making pictures: a film whose
 * picture is carried over untouched is repackaged rather than drawn, and the
 * tool answers nothing at all when asked how many a second it is drawing. The
 * speed stands on its own there, and it is the number that matters anyway.
 */
function asWork(
  working: Producing,
  t: (key: string, values?: Record<string, string | number>) => string,
): string {
  const speed = working.speed >= 10 ? Math.round(working.speed) : working.speed.toFixed(2);
  return working.pictures_a_second > 0
    ? t("facts.working_at", { pictures: working.pictures_a_second.toFixed(1), speed })
    : t("facts.working_speed", { speed });
}

/** A size in bytes, as somebody reads one. */
function asSize(bytes: number): string {
  const giga = bytes / 1_000_000_000;
  return giga >= 1 ? `${giga.toFixed(1)} GB` : `${Math.round(bytes / 1_000_000)} MB`;
}

/** One line of the panel: a name and what it is, or nothing at all. */
function Line({
  name,
  is,
  behind = false,
}: {
  name: string;
  is: string | null;
  /** Marks the one number on this panel that can be bad news. */
  behind?: boolean;
}) {
  if (is === null || is === "") {
    return null;
  }
  return (
    <div className="facts-line">
      <span className="facts-name">{name}</span>
      <span className={behind ? "facts-is facts-behind" : "facts-is"}>{is}</span>
    </div>
  );
}

interface Props {
  plan: PlaybackPlan;
  video: React.RefObject<HTMLVideoElement | null>;
  /** The session producing the film, when one is. Nothing for a film played
   *  as it lies on the disk, where no tool is at work to ask. */
  session: string | null;
  t: (key: string, values?: Record<string, string | number>) => string;
  onClose: () => void;
}

export function PlaybackFacts({ plan, video, session, t, onClose }: Props) {
  const [says, setSays] = useState<WhatTheBrowserSays | null>(null);
  const [working, setWorking] = useState<Producing | null>(null);
  const [calibration, setCalibration] = useState<CalibrationEntry[]>([]);

  /* Read once, on opening: this device's own calibration does not change
     while a film plays. Empty for a device nobody has optimized, which is
     silently left off the panel rather than shown as a row of nothing. */
  useEffect(() => {
    storedCalibration()
      .then(setCalibration)
      .catch(() => setCalibration([]));
  }, []);

  /* Looked at on a rhythm rather than on an event: pictures shown and pictures
     dropped only ever climb, and nothing fires when they do. */
  useEffect(() => {
    const look = () => {
      const element = video.current;
      if (element) {
        setSays(readTheElement(element));
      }
    };
    look();
    const ticking = window.setInterval(look, LOOK_EVERY_MS);
    return () => window.clearInterval(ticking);
  }, [video]);

  /* How hard the machine is working, which only the server knows. Asked for
     while this panel is open and not a moment longer: it is a request a
     second, and the rest of the time nobody is looking at the answer. */
  useEffect(() => {
    if (!session) {
      setWorking(null);
      return;
    }
    const controller = new AbortController();
    const ask = () => {
      api
        .preparation(session, controller.signal)
        .then((seen) => setWorking(seen.producing))
        .catch(() => {
          // A session swept away while the panel was open, or a request cut
          // off on the way out. The next look carries the same news.
        });
    };
    ask();
    const ticking = window.setInterval(ask, LOOK_EVERY_MS);
    return () => {
      window.clearInterval(ticking);
      controller.abort();
    };
  }, [session]);

  const { film, rebuild } = plan;
  const picture = film.picture;
  const sound = film.sound;

  /* The frame and the picture are different shapes only on a film that says
     part of its frame is not the picture, and that difference is the whole
     reason such a film is handled apart. Said only when it is true, because a
     line repeating the same two numbers teaches nobody anything. */
  const frame =
    picture && picture.frame_width !== null && picture.frame_height !== null
      ? `${picture.frame_width}x${picture.frame_height}`
      : null;

  return (
    <aside className="facts" aria-label={t("facts.title")}>
      <div className="facts-head">
        <strong>{t("facts.title")}</strong>
        <button className="player-button" onClick={onClose} aria-label={t("facts.close")}>
          ✕
        </button>
      </div>

      <div className="facts-blocks">
        {/* Across every column rather than shut inside one. What this block
            holds is a sentence about the whole decision and not a value beside
            a name, and in a column of its own it wrapped six lines deep and set
            the height of everything beside it. */}
        <section className="facts-block facts-block-across">
          <h3>{t("facts.stream")}</h3>
          <Line name={t("facts.method")} is={t(`playback.${plan.method}`)} />
          <Line
            name={t("facts.why")}
            is={
              plan.reasons.length > 0
                ? plan.reasons.map((reason) => t(`reason.${reason.code}`)).join(" · ")
                : null
            }
          />
          {/* The one number here that can be bad news, and the reason the
              maintainer asked for this panel: a machine producing the film
              more slowly than it plays will stop the picture, and nothing
              else on any screen says so before it happens. */}
          <Line
            name={t("facts.working")}
            is={working ? asWork(working, t) : null}
            behind={working !== null && working.speed < KEEPING_UP}
          />
        </section>

        {picture && (
          <section className="facts-block">
            <h3>{t("facts.picture")}</h3>
            <Line
              name={t("facts.held")}
              is={[
                picture.codec.toUpperCase(),
                picture.profile,
                `${picture.width}x${picture.height}`,
                picture.bit_depth ? `${picture.bit_depth} bit` : null,
                picture.hdr ? t(`facts.hdr.${picture.hdr}`) : null,
                picture.frame_rate ? `${picture.frame_rate.toFixed(3)} fps` : null,
                asRate(picture.bitrate),
              ]
                .filter(Boolean)
                .join(" · ")}
            />
            <Line name={t("facts.frame")} is={frame} />
            <Line
              name={t("facts.done")}
              is={
                rebuild
                  ? [
                      t(`player.rebuilt_by.${rebuild.by}`),
                      rebuild.codec.toUpperCase(),
                      rebuild.height !== null ? `${rebuild.height}p` : null,
                      asRate(rebuild.bitrate),
                    ]
                      .filter(Boolean)
                      .join(" · ")
                  : t("facts.carried_over")
              }
            />
            {says && (
              <>
                <Line name={t("facts.arrived")} is={`${says.across}x${says.down}`} />
                <Line name={t("facts.drawn")} is={`${says.drawnAcross}x${says.drawnDown}`} />
                <Line
                  name={t("facts.pictures")}
                  is={t("facts.pictures_count", { shown: says.shown, dropped: says.dropped })}
                />
                <Line
                  name={t("facts.held_ahead")}
                  is={
                    says.heldTo !== null && video.current
                      ? t("facts.seconds", {
                          count: Math.max(
                            0,
                            Math.round(says.heldTo - video.current.currentTime),
                          ),
                        })
                      : null
                  }
                />
              </>
            )}
          </section>
        )}

        {sound && (
          <section className="facts-block">
            <h3>{t("facts.sound")}</h3>
            <Line
              name={t("facts.held")}
              is={[
                sound.codec.toUpperCase(),
                sound.channel_layout ?? t("facts.channels", { count: sound.channels }),
                sound.sample_rate ? `${(sound.sample_rate / 1000).toFixed(1)} kHz` : null,
                asRate(sound.bitrate),
              ]
                .filter(Boolean)
                .join(" · ")}
            />
            <Line
              name={t("facts.done")}
              is={
                plan.method === "full_transcode" || plan.method === "transcode_audio"
                  ? t("facts.rebuilt_sound")
                  : t("facts.carried_over")
              }
            />
          </section>
        )}

        <section className="facts-block">
          <h3>{t("facts.file")}</h3>
          <Line name={t("facts.container")} is={film.container} />
          <Line name={t("facts.size")} is={asSize(film.size_bytes)} />
          <Line name={t("facts.rate")} is={asRate(film.overall_bitrate)} />
        </section>

        {calibration.length > 0 && (
          <section className="facts-block">
            <h3>{t("facts.calibration")}</h3>
            {calibration.map((entry) => (
              <Line
                key={entry.codec}
                name={entry.codec.toUpperCase()}
                is={
                  entry.usable
                    ? t("facts.calibration_at", { height: entry.tested_height })
                    : t("facts.calibration_unusable")
                }
              />
            ))}
          </section>
        )}
      </div>
    </aside>
  );
}
