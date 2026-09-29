/*
 * The bar of the player of music, at the foot of every page for as long as
 * there is something in the queue, and gone while a film plays.
 *
 * Laid out as the servers people come from lay it out: what is playing on
 * the left, the controls in the middle, the sound and the ways round the
 * queue on the right, and how far into the song along the top edge. On a
 * phone, what is playing and the two buttons a thumb reaches for, the rest
 * a press away on the page of what is playing.
 */

import { useEffect, useState } from "react";
import type { Picture } from "../../api";
import { asClock } from "../../clock";
import { useShownPicture } from "../../components/picture";
import { PlayIcon } from "../../icons";
import { useIsAFilmOnScreen } from "../../on-screen";
import { useSettings } from "../../settings";
import { namesOf } from "../tiles";
import { NextIcon, PauseIcon, PreviousIcon, QueueIcon, RepeatIcon, ShuffleIcon, StopIcon, VolumeIcon } from "./icons";
import { useMusic, useMusicTime } from "./player";
import type { Music } from "./player";

export function MusicBar() {
  const { t } = useSettings();
  const music = useMusic();
  const film = useIsAFilmOnScreen();
  const shown = music.song !== null && !film && !music.open;

  // Room kept at the foot of the pages, so the bar never hides the end of
  // one.
  useEffect(() => {
    const root = document.documentElement;
    if (shown) {
      root.dataset.musicBar = "";
    } else {
      delete root.dataset.musicBar;
    }
    return () => {
      delete root.dataset.musicBar;
    };
  }, [shown]);

  if (!shown || !music.song) {
    return null;
  }
  const song = music.song;

  return (
    <div className="music-bar-dock" role="region" aria-label={t("music.player")}>
      <Progress music={music} />
      <div className="music-bar-inner">
        <button
          type="button"
          className="music-bar-now"
          onClick={() => music.setOpen(true)}
          title={t("music.open_player")}
        >
          <Cover pictures={song.cover} />
          <span className="music-bar-words">
            <span className="music-bar-title">{song.title}</span>
            <span className="music-bar-artists">{namesOf(song.artists)}</span>
          </span>
        </button>

        <div className="music-bar-controls">
          <button type="button" className="music-control music-control-wide" onClick={music.previous} aria-label={t("music.previous")} title={t("music.previous")}>
            <PreviousIcon size={20} />
          </button>
          <PlayPause music={music} />
          <button type="button" className="music-control music-control-wide" onClick={music.stop} aria-label={t("music.stop")} title={t("music.stop")}>
            <StopIcon size={18} />
          </button>
          <button type="button" className="music-control" onClick={music.next} aria-label={t("music.next")} title={t("music.next")}>
            <NextIcon size={20} />
          </button>
          <Clock />
        </div>

        <div className="music-bar-side">
          <Loudness music={music} />
          <Ways music={music} />
          <button type="button" className="music-control" onClick={() => music.setOpen(true)} aria-label={t("music.queue")} title={t("music.queue")}>
            <QueueIcon size={20} />
          </button>
        </div>
      </div>
    </div>
  );
}

/** The cover of what is playing, or a plain square while there is none. */
export function Cover({ pictures, large = false }: { pictures: Picture[]; large?: boolean }) {
  const { picture, itDidNotLoad } = useShownPicture(pictures);
  return (
    <span className={`music-now-cover${large ? " music-now-cover-large" : ""}`}>
      {picture && (
        <img
          src={picture.src}
          srcSet={picture.srcSet || undefined}
          sizes={large ? "(max-width: 600px) 80vw, 420px" : "56px"}
          alt=""
          onError={itDidNotLoad}
        />
      )}
    </span>
  );
}

export function PlayPause({ music, large = false }: { music: Music; large?: boolean }) {
  const { t } = useSettings();
  const label = t(music.playing ? "music.pause" : "music.play");
  return (
    <button
      type="button"
      className={`music-control music-control-main${large ? " music-control-large" : ""}${music.waiting ? " music-control-waiting" : ""}`}
      onClick={music.toggle}
      aria-label={label}
      title={label}
    >
      {music.playing ? <PauseIcon size={large ? 30 : 22} /> : <PlayIcon size={large ? 30 : 22} />}
    </button>
  );
}

/** Where the song has got to, and how long it runs. */
export function Clock() {
  const { position, length } = useMusicTime();
  return (
    <span className="music-clock">
      {asClock(position)} / {asClock(length)}
    </span>
  );
}

/**
 * How far into the song, which a hand moves. While it is held the bar
 * follows the hand and the song stays where it is; letting go is the move.
 */
export function Progress({ music, standing = false }: { music: Music; standing?: boolean }) {
  const { t } = useSettings();
  const { position, length } = useMusicTime();
  const [held, setHeld] = useState<number | null>(null);
  const shown = held ?? position;
  const share = length > 0 ? Math.min(shown / length, 1) : 0;
  return (
    <input
      type="range"
      className={`music-progress${standing ? " music-progress-standing" : ""}`}
      min={0}
      max={Math.max(length, 1)}
      step={1}
      value={shown}
      style={{ ["--played" as string]: `${share * 100}%` }}
      aria-label={t("music.position")}
      aria-valuetext={`${asClock(shown)} / ${asClock(length)}`}
      onChange={(event) => setHeld(Number(event.target.value))}
      onPointerUp={() => {
        if (held !== null) {
          music.seek(held);
          setHeld(null);
        }
      }}
      onKeyUp={() => {
        if (held !== null) {
          music.seek(held);
          setHeld(null);
        }
      }}
    />
  );
}

export function Loudness({ music }: { music: Music }) {
  const { t } = useSettings();
  const { volume, muted } = music.loudness;
  const off = muted || volume === 0;
  return (
    <span className="music-loudness">
      <button
        type="button"
        className="music-control"
        onClick={() => music.setMuted(!muted)}
        aria-label={t(off ? "music.sound_on" : "music.sound_off")}
        title={t(off ? "music.sound_on" : "music.sound_off")}
      >
        <VolumeIcon off={off} size={20} />
      </button>
      <input
        type="range"
        className="music-volume"
        min={0}
        max={1}
        step={0.02}
        value={muted ? 0 : volume}
        style={{ ["--played" as string]: `${(muted ? 0 : volume) * 100}%` }}
        onChange={(event) => music.setVolume(Number(event.target.value))}
        aria-label={t("music.volume")}
      />
    </span>
  );
}

/** Shuffle and repeat, lit when they are on. */
export function Ways({ music }: { music: Music }) {
  const { t } = useSettings();
  const { shuffle, repeat } = music.queue;
  return (
    <>
      <button
        type="button"
        className={`music-control${shuffle ? " music-control-on" : ""}`}
        onClick={music.toggleShuffle}
        aria-pressed={shuffle}
        aria-label={t("music.shuffle")}
        title={t("music.shuffle")}
      >
        <ShuffleIcon size={20} />
      </button>
      <button
        type="button"
        className={`music-control${repeat !== "off" ? " music-control-on" : ""}`}
        onClick={music.cycleRepeat}
        aria-label={t(`music.repeat.${repeat}`)}
        title={t(`music.repeat.${repeat}`)}
      >
        <RepeatIcon one={repeat === "one"} size={20} />
      </button>
    </>
  );
}
