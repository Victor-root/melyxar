/*
 * The bar of the player of music, at the foot of every page for as long as
 * there is something in the queue, and gone while a film plays. It stays in
 * place under the page of what is playing, so opening and closing that page
 * moves nothing of the interface.
 *
 * Drawn as the player of films is drawn, with its bar, its clocks, its
 * buttons and its sound: the bar along the top with the time at both ends,
 * then what is playing on the left, the transport in the middle and the
 * sound and the ways round the queue on the right. On a phone, what is
 * playing and the two buttons a thumb reaches for, the rest a press away on
 * the page of what is playing.
 */

import { useEffect } from "react";
import type { Picture } from "../../api";
import { useShownPicture } from "../../components/picture";
import { useIsAFilmOnScreen } from "../../on-screen";
import { ICON } from "../../player/sound";
import { useSettings } from "../../settings";
import { namesOf } from "../tiles";
import { HeartButton, PlayButton, QueueButton, Rail, SongStepButton, Transport, Ways, Volume } from "./controls";
import { StopIcon } from "./icons";
import { useMusic } from "./player";

export function MusicBar() {
  const { t } = useSettings();
  const music = useMusic();
  const film = useIsAFilmOnScreen();
  const shown = music.song !== null && !film;

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
    <div className="music-bar-dock music-dark" role="region" aria-label={t("music.player")}>
      <Rail music={music} />
      <div className="music-bar-row">
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

        <div className="player-zone music-bar-transport">
          <Transport music={music} />
        </div>
        <div className="player-zone music-bar-phone">
          <PlayButton music={music} />
          <SongStepButton music={music} back={false} />
        </div>

        <div className="player-zone player-zone-bottom-right music-bar-tools">
          <HeartButton id={song.id} />
          <Volume music={music} />
          <Ways music={music} />
          <button type="button" className="player-button" onClick={music.stop} aria-label={t("music.stop")}>
            <StopIcon size={ICON - 4} />
          </button>
          <QueueButton music={music} />
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
