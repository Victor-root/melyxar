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

import { useEffect, useRef } from "react";
import type { Picture } from "../../api";
import { useShownPicture } from "../../components/picture";
import { useMediaQuery } from "../../media-query";
import { useIsAFilmOnScreen } from "../../on-screen";
import { useSettings } from "../../settings";
import { namesOf } from "../tiles";
import { HeartButton, PlayButton, QueueButton, Rail, SongStepButton, Transport, Ways, Volume } from "./controls";
import { useLeaving } from "./leaving";
import { useNowPlayingPage } from "./opening";
import { BAR_LEAVES_MS, useMusic } from "./player";
import { BehindThePlayer } from "./spectrum";

export function MusicBar() {
  const { t } = useSettings();
  const music = useMusic();
  const nowPlaying = useNowPlayingPage();
  const film = useIsAFilmOnScreen();
  const shown = music.song !== null && !film && !music.stopping;
  const reduced = useMediaQuery("(prefers-reduced-motion: reduce)");
  /* It goes down the way it came up when the music stops, still showing the
     last song; a film takes the screen at once, so it simply goes. */
  const leaving = useLeaving(shown, !film, reduced ? 0 : BAR_LEAVES_MS);
  // Room kept at the foot of the pages, so the bar never hides the end of
  // one. Kept until the bar has left: taken away as it starts to go down,
  // the whole page was laid out and drawn again during its way out.
  const there = shown || leaving;
  useEffect(() => {
    const root = document.documentElement;
    if (there) {
      root.dataset.musicBar = "";
    } else {
      delete root.dataset.musicBar;
    }
    return () => {
      delete root.dataset.musicBar;
    };
  }, [there]);
  const last = useRef(music.song);
  if (music.song) {
    last.current = music.song;
  }



  const song = shown ? music.song : last.current;
  if ((!shown && !leaving) || !song) {
    return null;
  }

  return (
    <div className={`music-bar-dock${leaving ? " music-bar-leaving" : ""}`} role="region" aria-label={t("music.player")}>
      <BehindThePlayer />
      <Rail music={music} />
      <div className="music-bar-row">
        <button
          type="button"
          className="music-bar-now"
          onClick={nowPlaying.marked ? nowPlaying.close : nowPlaying.open}
          title={t(nowPlaying.marked ? "music.close_player" : "music.open_player")}
        >
          <Cover pictures={song.cover} />
          <span className="music-bar-words">
            <span className="music-bar-title">{song.title}</span>
            <span className="music-bar-artists">{namesOf(song.artists)}</span>
          </span>
        </button>

        <div className="player-zone music-bar-transport">
          <Transport music={music} greyedWhenNone />
        </div>
        <div className="player-zone music-bar-phone">
          <SongStepButton music={music} back greyedWhenNone />
          <PlayButton music={music} />
          <SongStepButton music={music} back={false} greyedWhenNone />
        </div>

        <div className="player-zone player-zone-bottom-right music-bar-tools">
          <HeartButton id={song.id} />
          <Volume music={music} />
          <Ways music={music} />
          <QueueButton />
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
