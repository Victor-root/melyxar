/*
 * The remote: the account's other devices that can be reached, what plays
 * on each, and its buttons; and the choice of a device to play something on
 * (see the decision in docs/architecture/README.md).
 *
 * Everything shown comes from what the server says of each device, said
 * again on the page's line the moment it changes; between two words, where
 * a film has got to runs on by itself.
 */

import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from "react";
import type { ReactNode } from "react";
import { api } from "../api";
import type { PlaybackTrack, RemoteOrder, SteerableDevice, SteeredPlaying } from "../api";
import { refusalOf, useAsked } from "../asking";
import { asClock } from "../clock";
import { Modal } from "../components/modal";
import { Picker } from "../components/panel";
import { DeviceKindIcon } from "../components/device-lines";
import { deviceKind, deviceName } from "../devices";
import { refusalKey } from "../i18n";
import { PlayIcon, ScreenCastIcon } from "../icons";
import { NextIcon, PreviousIcon, RepeatIcon, ShuffleIcon, StopIcon, VolumeIcon } from "../music/player/icons";
import { useToast } from "../notifications/toasts";
import { PauseIcon, StepBackIcon, StepOnIcon } from "../player/icons";
import { trackName } from "../player/describe";
import { whichEpisode } from "../readable";
import { useSettings } from "../settings";
import { positionNow, useEverySecond, useSteerableDevices } from "./devices";

/** How far the step buttons go, back and on. */
const STEP_BACK_SECONDS = 10;
const STEP_ON_SECONDS = 30;

/** How long a handle is left still before what it says is sent. */
const SETTLED_AFTER_MS = 350;

/** Sends an order to a device, saying so when it did not arrive. Answers
 *  whether it did. */
function useSteer() {
  const { t } = useSettings();
  const toast = useToast();
  return useCallback(
    async (device: string, order: RemoteOrder) => {
      try {
        await api.steer(device, order);
        return true;
      } catch (error) {
        toast({ state: "trouble", title: t("remote.not_reached"), detail: t(refusalKey(refusalOf(error))) });
        return false;
      }
    },
    [t, toast],
  );
}

/** What plays, in one line under its title. */
function captionOf(playing: SteeredPlaying, t: ReturnType<typeof useSettings>["t"]): string | null {
  if (playing.kind === "song") {
    return [playing.artist, playing.album].filter(Boolean).join(" · ") || null;
  }
  if (playing.kind === "episode") {
    const which = whichEpisode(
      { season_number: playing.season, episode_number: playing.episode, title: playing.series ?? "" },
      t,
    );
    return which ?? playing.series;
  }
  return null;
}

/** The account's other devices, one line each, chosen by a press. */
function DeviceList({
  devices,
  onChoose,
}: {
  devices: SteerableDevice[];
  onChoose: (device: SteerableDevice) => void;
}) {
  const { t } = useSettings();
  if (devices.length === 0) {
    return <p className="remote-empty">{t("remote.none")}</p>;
  }
  return (
    <ul className="remote-devices">
      {devices.map((device) => (
        <li key={device.device}>
          <button type="button" className="remote-device" onClick={() => onChoose(device)}>
            <DeviceKindIcon kind={deviceKind(device.name, device.browser)} size={22} />
            <span className="remote-device-words">
              <span className="remote-device-name">{deviceName(device.name, t, device.browser)}</span>
              <span className="remote-device-playing">
                {device.playing === null
                  ? t("remote.nothing")
                  : t(device.playing.paused ? "remote.paused_on" : "remote.playing_on", {
                      title: device.playing.title,
                    })}
              </span>
            </span>
          </button>
        </li>
      ))}
    </ul>
  );
}

/** The tracks of a film, as a choice. */
function TrackChoice({
  label,
  tracks,
  chosen,
  withNone,
  onPick,
}: {
  label: string;
  tracks: PlaybackTrack[];
  chosen: string | null;
  withNone: boolean;
  onPick: (track: string | null) => void;
}) {
  const { t, language } = useSettings();
  const NONE = "";
  const options: [string, string][] = [
    ...(withNone ? [[NONE, t("player.no_subtitle")] as [string, string]] : []),
    ...tracks.map((track) => [track.id, trackName(track, t, language)] as [string, string]),
  ];
  return (
    <label className="remote-track">
      <span>{label}</span>
      <Picker
        label={label}
        value={chosen ?? NONE}
        options={options}
        onPick={(value) => onPick(value === NONE ? null : value)}
      />
    </label>
  );
}

/** A value sent once the hand settles, shown as it moves meanwhile. */
function useSettling(send: (value: number) => void): [number | null, (value: number) => void, () => void] {
  const [moving, setMoving] = useState<number | null>(null);
  const waiting = useRef<number | null>(null);
  const sendNow = useRef(send);
  sendNow.current = send;
  useEffect(() => () => {
    if (waiting.current !== null) window.clearTimeout(waiting.current);
  }, []);
  const move = useCallback((value: number) => {
    setMoving(value);
    if (waiting.current !== null) window.clearTimeout(waiting.current);
    waiting.current = window.setTimeout(() => {
      waiting.current = null;
      sendNow.current(value);
    }, SETTLED_AFTER_MS);
  }, []);
  const settled = useCallback(() => setMoving(null), []);
  return [moving, move, settled];
}

/** A line along which a value is chosen, what it stands for said at each
 *  end. Drawn as every slider of the interface is, across the whole width. */
function Line({
  label,
  value,
  max,
  step,
  onChange,
  start,
  end,
}: {
  label: string;
  value: number;
  max: number;
  step: number;
  onChange: (value: number) => void;
  start: ReactNode;
  end: ReactNode;
}) {
  const share = max > 0 ? Math.min(1, Math.max(0, value / max)) : 0;
  return (
    <div className="slider remote-line">
      <span className="remote-line-end">{start}</span>
      <span className="slider-rail remote-rail" style={{ ["--share" as string]: `${share}` }}>
        <input
          type="range"
          aria-label={label}
          min={0}
          max={max}
          step={step}
          value={Math.min(value, max)}
          onChange={(event) => onChange(Number(event.target.value))}
        />
      </span>
      <span className="remote-line-end">{end}</span>
    </div>
  );
}

/** What plays, with its picture and what it is about, as the card of the
 *  next episode shows one. */
function NowPlaying({ playing }: { playing: SteeredPlaying }) {
  const { t } = useSettings();
  const song = playing.kind === "song";
  const about = useAsked((signal) => (song ? Promise.resolve(null) : api.work(playing.work_id, signal)), [
    playing.work_id,
    song,
  ]).answer;
  const caption = captionOf(playing, t);
  return (
    <div className={song ? "remote-now remote-now-song" : "remote-now"}>
      {playing.picture && <img className="remote-picture" src={playing.picture} alt="" />}
      <div className="remote-now-words">
        {caption && <span className="remote-caption">{caption}</span>}
        <span className="remote-title">{playing.title}</span>
        {about?.overview && <p className="remote-overview">{about.overview}</p>}
      </div>
    </div>
  );
}

/** One device and its buttons. */
function Steering({ device, heardAt }: { device: SteerableDevice; heardAt: number }) {
  const { t } = useSettings();
  const steer = useSteer();
  const send = (order: RemoteOrder) => void steer(device.device, order);
  const playing = device.playing;
  const now = useEverySecond(playing !== null && !playing.paused);
  const [seeking, seekTo, seekHeard] = useSettling((seconds) => send({ do: "seek", seconds }));
  const [loudening, loudTo, loudHeard] = useSettling((level) => send({ do: "volume", level }));
  // What the device says next is where the handle stands from then on.
  useEffect(() => {
    seekHeard();
    loudHeard();
  }, [heardAt, seekHeard, loudHeard]);

  const volume = loudening ?? device.volume;
  const loudness =
    volume === null ? null : (
      <Line
        label={t("player.loudness")}
        value={volume}
        max={1}
        step={0.01}
        onChange={loudTo}
        start={
          <button
            type="button"
            className="remote-button remote-button-small"
            aria-label={t(device.muted ? "player.unmute" : "player.mute")}
            aria-pressed={device.muted === true}
            onClick={() => send({ do: "muted", muted: device.muted !== true })}
          >
            <VolumeIcon size={20} off={device.muted === true} />
          </button>
        }
        end={`${Math.round(volume * 100)} %`}
      />
    );

  if (playing === null) {
    return (
      <div className="remote-steering">
        <p className="remote-empty">{t("remote.nothing_here")}</p>
        {loudness}
      </div>
    );
  }
  const song = playing.kind === "song";
  const length = playing.duration_seconds;
  const position = seeking ?? positionNow(playing, heardAt, now);
  const queue = song ? device.queue : null;
  // A film on its own has nothing before or after it.
  const steps = song || playing.kind === "episode";
  return (
    <div className="remote-steering">
      <NowPlaying playing={playing} />
      {length !== null && (
        <Line
          label={t("player.position")}
          value={position}
          max={length}
          step={1}
          onChange={seekTo}
          start={asClock(position)}
          end={`-${asClock(Math.max(0, length - position))}`}
        />
      )}
      <div className="remote-buttons">
        {steps && (
          <button type="button" className="remote-button" aria-label={t("music.previous")} onClick={() => send({ do: "previous" })}>
            <PreviousIcon size={22} />
          </button>
        )}
        <button
          type="button"
          className="remote-button"
          aria-label={t("player.step_back", { seconds: STEP_BACK_SECONDS })}
          onClick={() => send({ do: "step", seconds: -STEP_BACK_SECONDS })}
        >
          <StepBackIcon size={24} seconds={STEP_BACK_SECONDS} />
        </button>
        <button
          type="button"
          className="remote-button remote-button-main"
          aria-label={t(playing.paused ? "player.play" : "player.pause")}
          onClick={() => send({ do: playing.paused ? "resume" : "pause" })}
        >
          {playing.paused ? <PlayIcon size={28} /> : <PauseIcon size={28} />}
        </button>
        <button
          type="button"
          className="remote-button"
          aria-label={t("player.step_on", { seconds: STEP_ON_SECONDS })}
          onClick={() => send({ do: "step", seconds: STEP_ON_SECONDS })}
        >
          <StepOnIcon size={24} seconds={STEP_ON_SECONDS} />
        </button>
        {steps && (
          <button type="button" className="remote-button" aria-label={t("music.next")} onClick={() => send({ do: "next" })}>
            <NextIcon size={22} />
          </button>
        )}
        <button type="button" className="remote-button" aria-label={t("music.stop")} onClick={() => send({ do: "stop" })}>
          <StopIcon size={20} />
        </button>
      </div>
      {loudness}
      {!song && (playing.audio.length > 1 || playing.subtitles.length > 0) && (
        <div className="remote-tracks">
          {playing.audio.length > 1 && (
            <TrackChoice
              label={t("work.audio")}
              tracks={playing.audio}
              chosen={playing.chosen_audio_id}
              withNone={false}
              onPick={(track) => track !== null && send({ do: "audio", track })}
            />
          )}
          {playing.subtitles.length > 0 && (
            <TrackChoice
              label={t("work.subtitles")}
              tracks={playing.subtitles}
              chosen={playing.chosen_subtitle_id}
              withNone
              onPick={(track) => send({ do: "subtitle", track })}
            />
          )}
        </div>
      )}
      {queue && (
        <>
          <div className="remote-buttons">
            <button
              type="button"
              className="remote-button"
              aria-label={t("music.shuffle")}
              aria-pressed={queue.shuffle}
              onClick={() => send({ do: "shuffle" })}
            >
              <ShuffleIcon size={20} />
            </button>
            <button
              type="button"
              className="remote-button"
              aria-label={t(`music.repeat.${queue.repeat}`)}
              aria-pressed={queue.repeat !== "off"}
              onClick={() => send({ do: "repeat" })}
            >
              <RepeatIcon size={20} one={queue.repeat === "one"} />
            </button>
          </div>
          <ol className="remote-queue" aria-label={t("music.queue")}>
            {queue.songs.map((one, place) => (
              <li key={`${place}-${one.id}`}>
                <button
                  type="button"
                  className="remote-queue-song"
                  aria-current={place === queue.at}
                  onClick={() => send({ do: "jump", index: place })}
                >
                  {one.title}
                </button>
              </li>
            ))}
          </ol>
        </>
      )}
    </div>
  );
}

/** The remote itself: the devices, then one of them. */
export function Remote({ onClose }: { onClose: () => void }) {
  const { t } = useSettings();
  const { devices, heardAt, cut } = useSteerableDevices();
  const [chosen, setChosen] = useState<string | null>(null);
  const device = devices?.find((one) => one.device === chosen) ?? null;
  return (
    <Modal
      title={device ? deviceName(device.name, t, device.browser) : t("nav.cast")}
      onBack={device ? () => setChosen(null) : undefined}
      onClose={onClose}
      className="remote"
    >
      {cut && devices === null ? (
        <p className="remote-empty">{t("remote.cut")}</p>
      ) : devices === null ? (
        <p className="remote-empty" aria-busy="true" />
      ) : device ? (
        <Steering device={device} heardAt={heardAt} />
      ) : (
        <DeviceList devices={devices} onChoose={(one) => setChosen(one.device)} />
      )}
    </Modal>
  );
}

/** The choice of a device to play something on, the order sent to the one
 *  chosen, made the moment it is chosen. `after` is done once it was sent,
 *  the music leaving this device for the other one. */
export function PlayOn({ order, onClose, after }: { order: () => RemoteOrder; onClose: () => void; after?: () => void }) {
  const { t } = useSettings();
  const toast = useToast();
  const steer = useSteer();
  const { devices, cut } = useSteerableDevices();
  const choose = async (device: SteerableDevice) => {
    if (await steer(device.device, order())) {
      toast({ state: "ok", title: t("remote.sent", { device: deviceName(device.name, t, device.browser) }) });
      after?.();
      onClose();
    }
  };
  return (
    <Modal title={t("remote.play_on")} onClose={onClose} className="remote">
      {cut && devices === null ? (
        <p className="remote-empty">{t("remote.cut")}</p>
      ) : devices === null ? (
        <p className="remote-empty" aria-busy="true" />
      ) : (
        <DeviceList devices={devices} onChoose={(device) => void choose(device)} />
      )}
    </Modal>
  );
}

/* Whether the remote is open, held apart from the button that opened it: a
   line of the account's menu goes with the menu, and the remote must not. */
const opened = { now: false, listeners: new Set<() => void>() };

function setOpened(now: boolean) {
  opened.now = now;
  for (const listener of opened.listeners) listener();
}

/** The remote, while it is open. Held once, by the page. */
export function TheRemote() {
  const open = useSyncExternalStore(
    (listener) => {
      opened.listeners.add(listener);
      return () => opened.listeners.delete(listener);
    },
    () => opened.now,
  );
  return open ? <Remote onClose={() => setOpened(false)} /> : null;
}

/** The remote's button, on the bar or as a line of the account's menu. */
export function RemoteButton({ inTheMenu }: { inTheMenu: boolean }) {
  const { t } = useSettings();
  const said = t("nav.cast");
  return (
    <button
      type="button"
      className={inTheMenu ? "header-menu-line" : "header-icon"}
      title={inTheMenu ? undefined : said}
      aria-label={inTheMenu ? undefined : said}
      onClick={() => setOpened(true)}
    >
      <ScreenCastIcon size={inTheMenu ? 16 : 24} />
      {inTheMenu && said}
    </button>
  );
}

/** A button that plays something on another device of the account. */
export function PlayOnButton({
  order,
  after,
  className,
}: {
  order: () => RemoteOrder;
  after?: () => void;
  className: string;
}) {
  const { t } = useSettings();
  const [open, setOpen] = useState(false);
  return (
    <>
      <button type="button" className={className} title={t("remote.play_on")} onClick={() => setOpen(true)}>
        <ScreenCastIcon size={22} />
        {t("remote.play_on_short")}
      </button>
      {open && <PlayOn order={order} after={after} onClose={() => setOpen(false)} />}
    </>
  );
}
