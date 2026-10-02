/*
 * Songs as a list: one line each, with the button that plays it, its number,
 * its cover, its name, who plays it, the album it is on, what can be done
 * with it and how long it runs. What does not fit on a line of actions goes
 * into the menu of the line.
 */

import { useLayoutEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { asClock } from "../clock";
import { useShownPicture } from "../components/picture";
import { PlayIcon } from "../icons";
import { PauseIcon } from "../player/icons";
import { useSettings } from "../settings";
import type { Credited, Song } from "./api";
import { Heart } from "./heart";
import { useGoneSongs } from "./marks";
import { PlayingWave } from "./playing-wave";
import { SongMenuButton, useSongActions } from "./song-menu";
import type { MenuLine } from "./song-menu";
import { useMusic } from "./player/player";

/** Who plays a song, each a way to their page. */
function Artists({ artists }: { artists: Credited[] }) {
  return (
    <>
      {artists.map((artist, index) => (
        <span key={artist.id}>
          {index > 0 && ", "}
          <Link to={`/music/artist/${artist.id}`}>{artist.name}</Link>
        </span>
      ))}
    </>
  );
}

/** How wide one button of a line is, gap included. */
const AN_ACTION = 36;

/** What a line keeps for everything but its actions: the buttons at its
 *  left, the cover, the name at the least, the heart, the menu and the
 *  length; and the album, on a line that has one. */
const KEPT_BESIDES = 560;
const KEPT_FOR_THE_ALBUM = 200;

/** The width below which the album is left out, as the stylesheet does. */
const NARROW = 600;

/**
 * How many actions each line of a list carries itself, from the width the
 * list has: as many as fit, the others going into the menu of the line.
 * Read once for the whole list, from its own width, so no line measures
 * anything.
 */
function useActionsThatFit(list: React.RefObject<HTMLOListElement | null>, showAlbum: boolean): number {
  const [fit, setFit] = useState(0);
  useLayoutEffect(() => {
    const element = list.current;
    if (!element) {
      return;
    }
    const measure = () => {
      const width = element.clientWidth;
      const album = showAlbum && width > NARROW ? KEPT_FOR_THE_ALBUM : 0;
      setFit(Math.max(0, Math.floor((width - KEPT_BESIDES - album) / AN_ACTION)));
    };
    measure();
    const watcher = new ResizeObserver(measure);
    watcher.observe(element);
    return () => watcher.disconnect();
  }, [list, showAlbum]);
  return fit;
}

function Cover({ song }: { song: Song }) {
  const { picture, itDidNotLoad } = useShownPicture(song.cover);
  return (
    <span className="music-song-cover">
      {picture && (
        <img
          src={picture.src}
          srcSet={picture.srcSet || undefined}
          sizes="44px"
          alt=""
          loading="lazy"
          decoding="async"
          draggable={false}
          onError={itDidNotLoad}
        />
      )}
    </span>
  );
}

/** What can be done with a song, on its line, as far as there is room. */
function Actions({ song, inline, extra }: { song: Song; inline: number; extra?: MenuLine[] }) {
  const { t } = useSettings();
  const { actions, dialog } = useSongActions([song], true);
  const carried = actions.slice(0, inline);
  return (
    <span className="music-song-actions">
      <Heart id={song.id} size={16} />
      {carried.map((action) => (
        <button
          key={action.key}
          type="button"
          className="music-more music-action"
          aria-label={action.said}
          title={action.said}
          onClick={action.act}
        >
          {action.mark}
        </button>
      ))}
      {dialog}
      <SongMenuButton
        songs={[song]}
        deletable
        label={t("music.more_about", { title: song.title })}
        extra={extra}
        inline={carried.length}
      />
    </span>
  );
}

export function SongList({
  songs,
  numbered,
  showAlbum = true,
  hideArtists,
  first = 0,
  onPlay,
  moreFor,
}: {
  songs: Song[];
  /** By the song's own number on its album, or by its place in the list. */
  numbered: "track" | "place";
  showAlbum?: boolean;
  /** Left out of each line when they are the album's own: an album page
      does not repeat its artist on every song. */
  hideArtists?: string;
  /** The place of the first line in the whole list. */
  first?: number;
  /** Plays the list from the line pressed. */
  onPlay?: (index: number) => void;
  /** What the menu of a line offers beyond what every song's does. */
  moreFor?: (index: number) => MenuLine[];
}) {
  const { t } = useSettings();
  const { song: playingNow, playing, toggle } = useMusic();
  const list = useRef<HTMLOListElement>(null);
  const inline = useActionsThatFit(list, showAlbum);
  const gone = useGoneSongs();
  return (
    <ol ref={list} className={`music-songs${showAlbum ? "" : " music-songs-no-album"}`}>
      {songs.map((song, index) => {
        if (gone.has(song.id)) {
          return null;
        }
        const artists =
          hideArtists !== undefined &&
          song.artists.map((artist) => artist.name).join(", ") === hideArtists
            ? []
            : song.artists;
        const current = playingNow?.id === song.id;
        const pausing = current && playing;
        const label = current ? t(playing ? "music.pause" : "music.play") : t("music.play_song", { title: song.title });
        return (
          <li
            className={`music-song music-song-full${current ? " music-song-playing" : ""}`}
            key={song.id}
            data-index={first + index}
          >
            <span className="music-song-lead">
              {(onPlay || current) && (
                <button
                  type="button"
                  className="music-song-toggle"
                  aria-label={label}
                  title={label}
                  onClick={() => (current ? toggle() : onPlay?.(index))}
                >
                  {pausing ? <PauseIcon size={20} /> : <PlayIcon size={20} />}
                </button>
              )}
            </span>
            <span className="music-song-number">
              {current ? <PlayingWave playing={playing} /> : numbered === "track" ? (song.track ?? "") : first + index + 1}
            </span>
            <Cover song={song} />
            <span className="music-song-words">
              <span className="music-song-title">{song.title}</span>
              {artists.length > 0 && (
                <span className="music-song-artists">
                  <Artists artists={artists} />
                </span>
              )}
            </span>
            {showAlbum && (
              <span className="music-song-album">
                {song.album && <Link to={`/music/album/${song.album.id}`}>{song.album.name}</Link>}
              </span>
            )}
            <Actions song={song} inline={inline} extra={moreFor?.(index)} />
            <span className="music-song-length" title={t("music.length")}>
              {song.seconds === null ? "" : asClock(song.seconds)}
            </span>
          </li>
        );
      })}
    </ol>
  );
}
