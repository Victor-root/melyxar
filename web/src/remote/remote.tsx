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
import { useBackClosesIt } from "../components/back-closes";
import { Modal } from "../components/modal";
import { pictureHeld, useFilledRoom } from "../fitting";
import { PHONE, useMediaQuery } from "../media-query";
import { Picker } from "../components/panel";
import { DeviceKindIcon } from "../components/device-lines";
import { deviceKind, deviceName } from "../devices";
import { refusalKey } from "../i18n";
import { PlayIcon, ScreenCastIcon } from "../icons";
import { NextIcon, PreviousIcon, RepeatIcon, ShuffleIcon, StopIcon, VolumeIcon } from "../music/player/icons";
import { useToast } from "../notifications/toasts";
import { AudioIcon, PauseIcon, StepBackIcon, StepOnIcon, SubtitlesIcon } from "../player/icons";
import { trackName } from "../player/describe";
import { whichEpisode } from "../readable";
import { useSettings } from "../settings";
import { positionNow, useEverySecond, useSteerableDevices } from "./devices";

/** How many steps the media volume of an Android telephone goes by. */
const ANDROID_STEPS = 15;

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
  mark,
}: {
  label: string;
  tracks: PlaybackTrack[];
  chosen: string | null;
  withNone: boolean;
  onPick: (track: string | null) => void;
  /** Drawn as a round button with this mark alone, its list rising from the
   *  foot of the screen, rather than as a field under its name. */
  mark?: ReactNode;
}) {
  const { t, language } = useSettings();
  const NONE = "";
  const options: [string, string][] = [
    ...(withNone ? [[NONE, t("player.no_subtitle")] as [string, string]] : []),
    ...tracks.map((track) => [track.id, trackName(track, t, language)] as [string, string]),
  ];
  const picker = (
    <Picker
      label={label}
      value={chosen ?? NONE}
      options={options}
      onPick={(value) => onPick(value === NONE ? null : value)}
      sheetOnAPhone={mark !== undefined}
      asAMark={mark === undefined ? undefined : { className: "remote-button remote-button-small", mark }}
    />
  );
  if (mark !== undefined) {
    return picker;
  }
  return (
    <label className="remote-track">
      <span>{label}</span>
      {picker}
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
      {end !== null && <span className="remote-line-end">{end}</span>}
    </div>
  );
}

/** The shapes a film's picture can be cut to on a telephone's remote, width
 *  over height, the widest and the tallest: a film's own is between them. */
const WIDEST_PICTURE = 2.4;
const TALLEST_PICTURE = 4 / 3;

/** What plays, with its picture and what it is about, as the card of the
 *  next episode shows one.
 *
 *  On a telephone a film fills what room the buttons leave and no more: its
 *  picture across the whole width, cut as tall as the whole synopsis under
 *  it lets it be, and the synopsis cut to the words that fit under the
 *  lowest picture when even that leaves too little room. Every button stays
 *  on the one screen, on any telephone. */
function NowPlaying({ playing, fitsTheScreen }: { playing: SteeredPlaying; fitsTheScreen: boolean }) {
  const { t } = useSettings();
  const song = playing.kind === "song";
  const about = useAsked((signal) => (song ? Promise.resolve(null) : api.work(playing.work_id, signal)), [
    playing.work_id,
    song,
  ]).answer;
  const caption = captionOf(playing, t);
  const box = useRef<HTMLDivElement>(null);
  const overview = about?.overview ?? null;
  const filled = useFilledRoom(
    box,
    overview,
    ".remote-overview",
    ".remote-picture",
    WIDEST_PICTURE,
    TALLEST_PICTURE,
    fitsTheScreen,
  );
  return (
    <div ref={box} className={song ? "remote-now remote-now-song" : "remote-now"}>
      {playing.picture && (
        <img
          className="remote-picture"
          src={playing.picture}
          alt=""
          style={filled ? pictureHeld(filled.pictureHeight) : undefined}
        />
      )}
      <div className="remote-now-words">
        {caption && <span className="remote-caption">{caption}</span>}
        <span className="remote-title">{playing.title}</span>
        {overview && <p className="remote-overview">{filled?.words ?? overview}</p>}
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
  const onAPhone = useMediaQuery(PHONE);
  const now = useEverySecond(playing !== null && !playing.paused);
  const [seeking, seekTo, seekHeard] = useSettling((seconds) => send({ do: "seek", seconds }));
  const [loudening, loudTo, loudHeard] = useSettling((level) => send({ do: "volume", level }));
  // What the device says next is where the handle stands from then on.
  useEffect(() => {
    seekHeard();
    loudHeard();
  }, [heardAt, seekHeard, loudHeard]);

  const volume = loudening ?? device.volume;
  /* An Android television or telephone goes by steps, as its own bar of
     volume does: the television says how many, a telephone's media go by
     fifteen. A computer goes by any amount, said as a share. */
  const volumeSteps =
    device.volume_steps ?? (deviceKind(device.name, device.browser) !== "computer" ? ANDROID_STEPS : null);
  /* The sound's line, with whatever stands at its right end after it. */
  const loudness = (beside: ReactNode = null) =>
    volume === null ? null : (
      <Line
        label={t("player.loudness")}
        value={volumeSteps === null ? volume : Math.round(volume * volumeSteps)}
        max={volumeSteps ?? 1}
        step={volumeSteps === null ? 0.01 : 1}
        onChange={(value) => loudTo(volumeSteps === null ? value : value / volumeSteps)}
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
        end={
          volumeSteps === null || beside !== null ? (
            <>
              {volumeSteps === null && <span className="remote-volume-said">{`${Math.round(volume * 100)} %`}</span>}
              {beside}
            </>
          ) : null
        }
      />
    );

  if (playing === null) {
    return (
      <div className="remote-steering">
        <p className="remote-empty">{t("remote.nothing_here")}</p>
        {loudness()}
      </div>
    );
  }
  const song = playing.kind === "song";
  const length = playing.duration_seconds;
  const position = seeking ?? positionNow(playing, heardAt, now);
  const queue = song ? device.queue : null;
  // A film on its own has nothing before or after it.
  const steps = song || playing.kind === "episode";
  // On a telephone it would crowd the buttons out from under the thumb.
  const stopByTheLine = onAPhone && length !== null;
  const stop = (
    <button
      type="button"
      className={stopByTheLine ? "remote-button remote-button-small" : "remote-button"}
      aria-label={t("music.stop")}
      onClick={() => send({ do: "stop" })}
    >
      <StopIcon size={20} />
    </button>
  );
  const left = length === null ? "" : `-${asClock(Math.max(0, length - position))}`;
  const choices = (asMarks: boolean) => (
    <>
      {playing.audio.length > 1 && (
        <TrackChoice
          label={t("work.audio")}
          tracks={playing.audio}
          chosen={playing.chosen_audio_id}
          withNone={false}
          onPick={(track) => track !== null && send({ do: "audio", track })}
          mark={asMarks ? <AudioIcon size={20} /> : undefined}
        />
      )}
      {playing.subtitles.length > 0 && (
        <TrackChoice
          label={t("work.subtitles")}
          tracks={playing.subtitles}
          chosen={playing.chosen_subtitle_id}
          withNone
          onPick={(track) => send({ do: "subtitle", track })}
          mark={asMarks ? <SubtitlesIcon size={20} /> : undefined}
        />
      )}
    </>
  );
  const hasChoices = !song && (playing.audio.length > 1 || playing.subtitles.length > 0);
  /* On a telephone a film's tracks are two round buttons at the end of the
     sound's line, each opening its list from the foot of the screen, as the
     player's own: the room they took as two fields goes to the film. */
  const choicesBySound = onAPhone && hasChoices;
  return (
    <div className={song ? "remote-steering" : "remote-steering remote-steering-fits"}>
      <NowPlaying playing={playing} fitsTheScreen={onAPhone && !song} />
      {length !== null && (
        <Line
          label={t("player.position")}
          value={position}
          max={length}
          step={1}
          onChange={seekTo}
          start={asClock(position)}
          end={
            stopByTheLine ? (
              <>
                {left}
                {stop}
              </>
            ) : (
              left
            )
          }
        />
      )}
      <div className="remote-controls">
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
          {!stopByTheLine && stop}
        </div>
        {loudness(choicesBySound ? choices(true) : null)}
        {choicesBySound && volume === null && <div className="remote-choice-marks">{choices(true)}</div>}
      </div>
      {hasChoices && !choicesBySound && <div className="remote-tracks">{choices(false)}</div>}
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
export function Remote({ startOn, onClose }: { startOn: string | null; onClose: () => void }) {
  const { t } = useSettings();
  const { devices, heardAt, cut } = useSteerableDevices();
  const [chosen, setChosen] = useState<string | null>(startOn);
  const device = devices?.find((one) => one.device === chosen) ?? null;
  /* A device chosen is a step of its own in the history, put in at the
     press: added later, on the way back from it, a phone's back key would
     step over it and leave the application. */
  const leaveTheDevice = useBackClosesIt(() => setChosen(null), chosen !== null);
  return (
    <Modal
      title={device ? deviceName(device.name, t, device.browser) : t("nav.cast")}
      onBack={device ? () => leaveTheDevice(() => setChosen(null)) : undefined}
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
 *  chosen, made the moment it is chosen, and the remote opened on it ready to
 *  steer what it was sent. `after` is done once it was sent, the music
 *  leaving this device for the other one. */
export function PlayOn({ order, onClose, after }: { order: () => RemoteOrder; onClose: () => void; after?: () => void }) {
  const { t } = useSettings();
  const steer = useSteer();
  const { devices, cut } = useSteerableDevices();
  const choose = async (device: SteerableDevice) => {
    if (await steer(device.device, order())) {
      after?.();
      onClose();
      openTheRemote(device.device);
    }
  };
  return (
    <Modal title={t("remote.play_on")} onClose={onClose} className="remote-choice">
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
   line of the account's menu goes with the menu, and the remote must not.
   The device it opened on, when it opened on one. */
const opened: { now: { on: string | null } | null; listeners: Set<() => void> } = {
  now: null,
  listeners: new Set(),
};

function setOpened(now: { on: string | null } | null) {
  opened.now = now;
  for (const listener of opened.listeners) listener();
}

/** Opens the remote, on this device when one is given. */
export function openTheRemote(on: string | null = null) {
  setOpened({ on });
}

/** Whether the remote is open. */
export function useRemoteOpen(): boolean {
  return useSyncExternalStore(
    (listener) => {
      opened.listeners.add(listener);
      return () => opened.listeners.delete(listener);
    },
    () => opened.now !== null,
  );
}

/** The remote, while it is open. Held once, by the page. */
export function TheRemote() {
  const open = useRemoteOpen();
  return open ? <Remote startOn={opened.now?.on ?? null} onClose={() => setOpened(null)} /> : null;
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
      onClick={() => openTheRemote()}
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
  round,
}: {
  order: () => RemoteOrder;
  after?: () => void;
  className: string;
  /** Its picture alone, among the round marks of a work. */
  round?: boolean;
}) {
  const { t } = useSettings();
  const [open, setOpen] = useState(false);
  return (
    <>
      <button
        type="button"
        className={className}
        onClick={() => setOpen(true)}
        aria-label={round ? t("remote.play_on") : undefined}
        title={round ? t("remote.play_on") : undefined}
      >
        <ScreenCastIcon size={22} />
        {!round && t("remote.play_on")}
      </button>
      {open && <PlayOn order={order} after={after} onClose={() => setOpen(false)} />}
    </>
  );
}
