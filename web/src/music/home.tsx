/*
 * The newest albums of each library of music, for the home page: its row,
 * and the covers fanned out on its tile in the band of ways in.
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
import { useSettings } from "../settings";
import { music } from "./api";
import type { Album } from "./api";
import { AlbumTile } from "./tiles";

/** As many as a row of films holds. */
const ON_A_SHELF = 20;

const NO_ALBUMS: Record<string, Album[]> = {};

/** The newest albums of each library of music, by library, read once for the
 *  whole home page and again whenever one of `readAgain` changes. Kept for
 *  when the page is walked back to, like the rest of it. */
export function useNewestAlbums(libraries: Library[], readAgain: unknown[]): Record<string, Album[]> {
  const ids = libraries
    .filter((library) => library.kind === "music")
    .map((library) => library.id)
    .join(",");

  const asked = useAsked(
    async (signal) => {
      const each = ids ? ids.split(",") : [];
      const pages = await Promise.all(
        each.map((library) => music.albums(library, "added", false, 0, ON_A_SHELF, {}, signal)),
      );
      return Object.fromEntries(each.map((library, at) => [library, pages[at].items]));
    },
    [ids, ...readAgain],
    `home-albums:${ids}`,
  );
  return asked.answer ?? NO_ALBUMS;
}

export function NewestMusic({ library, albums }: { library: Library; albums: Album[] }) {
  const { t } = useSettings();
  if (albums.length === 0) {
    return null;
  }
  return (
    <section className="section">
      <RowHead
        mark={<KindIcon kind="music" size={24} />}
        title={t("home.newest.named", { name: library.name })}
        to={`/library/${library.id}`}
      />
      <Row>
        {albums.map((album) => (
          <AlbumTile key={album.id} album={album} />
        ))}
      </Row>
    </section>
  );
}
