/*
 * Songs as a list: one line each, with its number, its name, who plays it,
 * the album it is on and how long it runs.
 */

import { Link } from "react-router-dom";
import { asClock } from "../clock";
import { useSettings } from "../settings";
import type { Credited, Song } from "./api";
import { Heart } from "./heart";
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

export function SongList({
  songs,
  numbered,
  showAlbum = true,
  hideArtists,
  first = 0,
  onPlay,
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
}) {
  const { t } = useSettings();
  const { song: playingNow } = useMusic();
  return (
    <ol className={`music-songs${showAlbum ? "" : " music-songs-no-album"}`}>
      {songs.map((song, index) => {
        const artists =
          hideArtists !== undefined &&
          song.artists.map((artist) => artist.name).join(", ") === hideArtists
            ? []
            : song.artists;
        return (
          <li
            className={`music-song${playingNow?.id === song.id ? " music-song-playing" : ""}`}
            key={song.id}
            data-index={first + index}
          >
            <span className="music-song-number">
              {numbered === "track" ? (song.track ?? "") : first + index + 1}
            </span>
            <span className="music-song-words">
              {onPlay ? (
                <button
                  type="button"
                  className="music-song-title music-song-play"
                  onClick={() => onPlay(index)}
                  title={t("music.play_song", { title: song.title })}
                >
                  {song.title}
                </button>
              ) : (
                <span className="music-song-title">{song.title}</span>
              )}
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
            <Heart id={song.id} size={16} />
            <span className="music-song-length" title={t("music.length")}>
              {song.seconds === null ? "" : asClock(song.seconds)}
            </span>
          </li>
        );
      })}
    </ol>
  );
}
