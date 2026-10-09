/*
 * Songs as a list: one line each, with the button that plays it, its number,
 * its cover, its name, who plays it, the album it is on, what can be done
 * with it and how long it runs. What does not fit on a line of actions goes
 * into the menu of the line.
 */

import { memo, useCallback, useLayoutEffect, useRef, useState } from "react";
import { asClock } from "../clock";
import { useShownPicture } from "../components/picture";
import { HeartIcon, PlayIcon } from "../icons";
import { PHONE, useMediaQuery } from "../media-query";
import { QuietLink } from "../navigating";
import { PauseIcon } from "../player/icons";
import { useSettings } from "../settings";
import type { Credited, Song } from "./api";
import { Heart } from "./heart";
import { useGoneSongs, useLiking } from "./marks";
import { PlayingWave } from "./playing-wave";
import { SongMenuButton, useSongActions } from "./song-menu";
import type { MenuLine } from "./song-menu";
import { useMusicControls, useMusicNow } from "./player/player";

/** Who plays a song, each a way to their page. */
function Artists({ artists, plain }: { artists: Credited[]; plain?: boolean }) {
  return (
    <>
      {artists.map((artist, index) => (
        <span key={artist.id}>
          {index > 0 && ", "}
          {plain ? artist.name : <QuietLink to={`/music/artist/${artist.id}`}>{artist.name}</QuietLink>}
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
function useActionsThatFit(
  list: React.RefObject<HTMLOListElement | null>,
  showAlbum: boolean,
  wanted: boolean,
): number {
  const [fit, setFit] = useState(0);
  useLayoutEffect(() => {
    const element = list.current;
    if (!element || !wanted) {
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
  }, [list, showAlbum, wanted]);
  return wanted ? fit : 0;
}

export function Cover({ song }: { song: Song }) {
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
function Actions({
  song,
  inline,
  extra,
  heartInMenu,
}: {
  song: Song;
  inline: number;
  extra?: MenuLine[];
  heartInMenu: boolean;
}) {
  const { t } = useSettings();
  const { liked, setLiked } = useLiking(song.id);
  const { actions, dialog } = useSongActions([song], true);
  const carried = actions.slice(0, inline);
  const heart: MenuLine = {
    key: "heart",
    said: t(liked ? "card.unfavourite" : "card.favourite"),
    mark: <HeartIcon size={17} filled={liked} />,
    act: () => setLiked(!liked),
  };
  return (
    <span className="music-song-actions">
      {!heartInMenu && <Heart id={song.id} size={16} />}
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
        extra={heartInMenu ? [heart, ...(extra ?? [])] : extra}
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
  menuOnly,
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
  /** Every action but the heart goes into the menu of the line, whatever the
      room: the name keeps all of it. */
  menuOnly?: boolean;
  /** The place of the first line in the whole list. */
  first?: number;
  /** Plays the list from the line pressed. */
  onPlay?: (index: number) => void;
  /** What the menu of a line offers beyond what every song's does. */
  moreFor?: (index: number) => MenuLine[];
}) {
  const list = useRef<HTMLOListElement>(null);
  /* On a phone every list is drawn as the lines of what was listened to: no
     number, no length and no heart on the line, the name taking the room,
     the whole line playing it and everything else in its menu. Laid out as
     on a computer, a name had five letters left. */
  const compact = useMediaQuery(PHONE);
  /* A list whose lines put every action in their menu has nothing to
     measure. */
  const inline = useActionsThatFit(list, showAlbum, !menuOnly && !compact);
  const gone = useGoneSongs();
  /* One way to play for the whole list, the same from one drawing to the
     next, so a line is drawn again only when its own song is. */
  const playNow = useRef(onPlay);
  useLayoutEffect(() => {
    playNow.current = onPlay;
  });
  const play = useCallback((index: number) => playNow.current?.(index), []);
  return (
    <ol ref={list} className={`music-songs${showAlbum ? "" : " music-songs-no-album"}${compact ? " music-songs-compact" : ""}`}>
      {songs.map((song, index) =>
        gone.has(song.id) ? null : (
          <SongLine
            key={song.id}
            song={song}
            index={index}
            first={first}
            numbered={numbered}
            showAlbum={showAlbum}
            hideArtists={hideArtists}
            compact={compact}
            inline={inline}
            onPlay={onPlay ? play : undefined}
            extra={moreFor?.(index)}
          />
        ),
      )}
    </ol>
  );
}

/** One line of a list. It watches on its own whether its song is the one
 *  playing, so a song starting or pausing draws two lines again, not every
 *  list on the screen. A list put in order by hand holds each line in one of
 *  its own, so there the line is not a list item itself. */
export const SongLine = memo(function SongLine({
  song,
  index,
  first,
  numbered,
  showAlbum,
  hideArtists,
  compact,
  inline,
  onPlay,
  extra,
  inItsOwnLine = false,
}: {
  song: Song;
  index: number;
  first: number;
  numbered: "track" | "place";
  showAlbum: boolean;
  hideArtists?: string;
  compact?: boolean;
  inline: number;
  onPlay?: (index: number) => void;
  extra?: MenuLine[];
  inItsOwnLine?: boolean;
}) {
  const { t } = useSettings();
  const { toggle } = useMusicControls();
  const state = useMusicNow((now, playing) => (now?.id !== song.id ? "other" : playing ? "playing" : "paused"));
  const current = state !== "other";
  const playing = state === "playing";
  const artists =
    hideArtists !== undefined && song.artists.map((artist) => artist.name).join(", ") === hideArtists
      ? []
      : song.artists;
  const label = current ? t(playing ? "music.pause" : "music.play") : t("music.play_song", { title: song.title });
  const Line = inItsOwnLine ? "div" : "li";
  return (
    <Line
      className={`music-song music-song-full${current ? " music-song-playing" : ""}${inItsOwnLine ? " music-song-in-a-playlist" : ""}`}
      data-index={first + index}
      onClick={
        compact
          ? (event) => {
              const target = event.target as HTMLElement;
              /* What a menu opened from the line draws elsewhere, and its
                 buttons do something of their own: neither is a press on the
                 line. */
              if (!event.currentTarget.contains(target) || target.closest("button")) {
                return;
              }
              if (current) {
                toggle();
              } else {
                onPlay?.(index);
              }
            }
          : undefined
      }
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
            {playing ? <PauseIcon size={20} /> : <PlayIcon size={20} />}
          </button>
        )}
      </span>
      {!compact && (
        <span className="music-song-number">
          {current ? <PlayingWave playing={playing} /> : numbered === "track" ? (song.track ?? "") : first + index + 1}
        </span>
      )}
      <Cover song={song} />
      <span className="music-song-words">
        <span className="music-song-title">{song.title}</span>
        {artists.length > 0 && (
          <span className="music-song-artists">
            <Artists artists={artists} plain={compact} />
          </span>
        )}
      </span>
      {showAlbum && (
        <span className="music-song-album">
          {song.album &&
            (compact ? song.album.name : <QuietLink to={`/music/album/${song.album.id}`}>{song.album.name}</QuietLink>)}
        </span>
      )}
      <Actions song={song} inline={inline} extra={extra} heartInMenu={!!compact} />
      {!compact && (
        <span className="music-song-length" title={t("music.length")}>
          {song.seconds === null ? "" : asClock(song.seconds)}
        </span>
      )}
    </Line>
  );
});
