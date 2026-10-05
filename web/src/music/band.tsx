/*
 * The tile of music in the band of ways in, under the banner: the newest
 * album covers fanned out on the same water as the other tiles, square as a
 * cover is.
 *
 * Drawn by music with music's own albums, and handed to the band, which only
 * gives it its place after the others: the band knows nothing of albums.
 */

import { useState } from "react";
import { Link } from "react-router-dom";
import type { Library } from "../api";
import { BandCover } from "../components/band";
import { useShownPicture } from "../components/picture";
import { ChevronRightIcon, KindIcon } from "../icons";
import { nameOfKind, whereAKindLeads } from "../libraries";
import { useSettings } from "../settings";
import type { Album } from "./api";

/** As many covers as the other tiles fan out posters. */
const IN_A_FAN = 5;

/** How many covers, from the middle out, have a record behind them. */
const WITH_A_RECORD = 3;

/** What the tile fans out: the newest albums with a cover first, as the
 *  other tiles put their posters ahead of the holes. */
export function coversOf(albums: Album[]): Album[] {
  const drawn = albums.filter((album) => album.cover.length > 0);
  const bare = albums.filter((album) => album.cover.length === 0);
  return [...drawn, ...bare].slice(0, IN_A_FAN);
}

export function MusicBandTile({ libraries, albums }: { libraries: Library[]; albums: Album[] }) {
  const { t } = useSettings();
  const fan = coversOf(albums);
  // The records are drawn the first time the pointer or the keyboard comes
  // to the tile, since nothing of them is seen before.
  const [woken, setWoken] = useState(false);
  if (!libraries.some((library) => library.kind === "music")) {
    return null;
  }
  return (
    <Link
      className="band-tile band-tile-music"
      to={whereAKindLeads("music", libraries)}
      onPointerEnter={() => setWoken(true)}
      onFocus={() => setWoken(true)}
      style={{ ["--tile-color" as string]: fan[0]?.color ?? "var(--surface-raised)" }}
    >
      {fan.length > 0 ? (
        <>
          <span className="band-ghost" aria-hidden="true">
            <KindIcon kind="music" />
          </span>
          <span className="band-halo" aria-hidden="true" />
          <BandCover pictures={fan[0].cover} />
          <Fan fan={fan} mirror />
          <Fan fan={fan} records={woken} />
        </>
      ) : (
        <span className="band-mark" aria-hidden="true">
          <KindIcon kind="music" />
        </span>
      )}
      <span className="band-words">
        <span className="band-name">
          <KindIcon kind="music" size={17} />
          {nameOfKind("music", libraries, t)}
        </span>
      </span>
      <span className="band-arrow" aria-hidden="true">
        <ChevronRightIcon size={18} />
      </span>
    </Link>
  );
}

/** Never in the floor's reflection: a filter blurs and ripples it, and a
 *  turning record under it would have that filter redone every frame. */
function Fan({ fan, mirror, records }: { fan: Album[]; mirror?: boolean; records?: boolean }) {
  return (
    <span className={`band-posters band-fan-${fan.length}${mirror ? " band-mirror" : ""}`} aria-hidden="true">
      {fan.map((album, at) => (
        <Cover key={album.id} album={album} record={records === true && at < WITH_A_RECORD} />
      ))}
    </span>
  );
}

function Cover({ album, record }: { album: Album; record: boolean }) {
  const { picture, itDidNotLoad } = useShownPicture(album.cover);
  return (
    <span className="band-poster" style={{ ["--card-color" as string]: album.color ?? "var(--surface-raised)" }}>
      {record && <span className="band-record" />}
      {picture ? (
        <img
          src={picture.src}
          srcSet={picture.srcSet || undefined}
          sizes="(max-width: 900px) 22vw, 130px"
          alt=""
          loading="lazy"
          decoding="async"
          draggable={false}
          onError={itDidNotLoad}
        />
      ) : (
        <span className="band-poster-initial">{album.title.slice(0, 1)}</span>
      )}
    </span>
  );
}
