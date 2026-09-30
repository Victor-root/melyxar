/*
 * Songs as wide tiles, two above two in a row that scrolls sideways: the
 * cover, the title and whose it is. How the songs listened to lately and
 * most are laid out, so a glance takes in a dozen of them.
 */

import { Row } from "../components/row";
import { useShownPicture } from "../components/picture";
import { PlayIcon } from "../icons";
import { useSettings } from "../settings";
import type { Song } from "./api";
import { namesOf } from "./tiles";

/** How many tiles stand one above the other. */
const PER_COLUMN = 2;

export function SongTiles({ songs, onPlay }: { songs: Song[]; onPlay: (index: number) => void }) {
  const columns = Array.from({ length: Math.ceil(songs.length / PER_COLUMN) }, (_, column) =>
    songs.slice(column * PER_COLUMN, (column + 1) * PER_COLUMN),
  );
  return (
    <Row>
      {columns.map((pair, column) => (
        <div key={pair[0].id} className="music-lines" data-card>
          {pair.map((song, at) => (
            <SongTile key={song.id} song={song} onPlay={() => onPlay(column * PER_COLUMN + at)} />
          ))}
        </div>
      ))}
    </Row>
  );
}

function SongTile({ song, onPlay }: { song: Song; onPlay: () => void }) {
  const { t } = useSettings();
  const { picture, itDidNotLoad } = useShownPicture(song.cover);
  return (
    <button type="button" className="music-line" onClick={onPlay} title={song.title}>
      <span className="music-line-cover">
        {picture && (
          <img
            src={picture.src}
            srcSet={picture.srcSet || undefined}
            sizes="80px"
            alt=""
            loading="lazy"
            decoding="async"
            draggable={false}
            onError={itDidNotLoad}
          />
        )}
        <span className="music-line-play" aria-label={t("music.play")}>
          <PlayIcon size={20} />
        </span>
      </span>
      <span className="music-line-words">
        <span className="music-line-title">{song.title}</span>
        <span className="music-line-note">{namesOf(song.artists)}</span>
      </span>
    </button>
  );
}
