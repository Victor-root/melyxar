/*
 * The newest albums, for the home page: its row, and the covers fanned out
 * on the tile of music in the band of ways in.
 *
 * Asked of music's own requests and drawn with music's own tiles: an album is
 * not a film's card, and the home page only gives it its place among the
 * others. Read again each time the home page is, so a scan that files albums
 * shows them here without the page being reloaded.
 */

import type { Library } from "../api";
import { useAsked } from "../asking";
import { Row, RowHead } from "../components/row";
import { KindIcon } from "../icons";
import { newestOfKind, whereAKindLeads } from "../libraries";
import { useSettings } from "../settings";
import { music } from "./api";
import type { Album } from "./api";
import { AlbumTile } from "./tiles";

/** As many as a row of films holds. */
const ON_A_SHELF = 20;

/** The newest albums of several libraries as one row, taken in turn from
 *  each so that no library pushes the others out of it. */
export function inTurn<T>(lists: T[][], room: number): T[] {
  const taken: T[] = [];
  for (let place = 0; taken.length < room && lists.some((list) => place < list.length); place += 1) {
    for (const list of lists) {
      if (place < list.length && taken.length < room) {
        taken.push(list[place]);
      }
    }
  }
  return taken;
}

const NO_ALBUMS: Album[] = [];

/** The newest albums of every library of music, read once for the whole
 *  home page and again whenever one of `readAgain` changes. Kept for when
 *  the page is walked back to, like the rest of it. */
export function useNewestAlbums(libraries: Library[], readAgain: unknown[]): Album[] {
  const ids = libraries
    .filter((library) => library.kind === "music")
    .map((library) => library.id)
    .join(",");

  const asked = useAsked(
    async (signal) => {
      const pages = await Promise.all(
        (ids ? ids.split(",") : []).map((library) =>
          music.albums(library, "added", false, 0, ON_A_SHELF, {}, signal),
        ),
      );
      return inTurn(pages.map((page) => page.items), ON_A_SHELF);
    },
    [ids, ...readAgain],
    `home-albums:${ids}`,
  );
  return asked.answer ?? NO_ALBUMS;
}

export function NewestMusic({ libraries, albums }: { libraries: Library[]; albums: Album[] }) {
  const { t } = useSettings();
  if (albums.length === 0) {
    return null;
  }
  return (
    <section className="section">
      <RowHead
        mark={<KindIcon kind="music" size={24} />}
        title={newestOfKind("music", libraries, t)}
        to={whereAKindLeads("music", libraries)}
      />
      <Row>
        {albums.map((album) => (
          <AlbumTile key={album.id} album={album} />
        ))}
      </Row>
    </section>
  );
}
