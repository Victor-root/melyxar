/*
 * Searching, which is the library grid with the query in the address.
 *
 * Reusing the grid rather than building a second one means a search result
 * sorts, filters and pages exactly like everything else. What the words find
 * in the music is drawn above it, or alone when the search is narrowed to
 * music, which has no grid of films to show.
 */

import { useSearchParams } from "react-router-dom";
import { LibraryPage } from "./library";
import type { Library } from "../api";
import {
  MusicFound,
  foundAny,
  musicScopeOf,
  useMusicFound,
} from "../music/search";
import { useSettings } from "../settings";
import { useTabPage } from "../tab-page";

export function SearchPage({ libraries }: { libraries: Library[] }) {
  const { t } = useSettings();
  const [parameters] = useSearchParams();
  const words = parameters.get("search") ?? "";
  const scope = musicScopeOf(parameters.get("in") ?? "", libraries);
  const found = useMusicFound(words, scope.looks, scope.library);
  /* The grid names the tab itself, except where only music is looked for. */
  useTabPage(scope.only ? t("nav.search") : null);

  if (scope.only) {
    return (
      <main className="page">
        <div className="section-head">
          <h1>{t("music.tabs")}</h1>
        </div>
        {found && !foundAny(found) && (
          <p className="notice">{t("library.empty")}</p>
        )}
        <MusicFound found={found} />
      </main>
    );
  }
  return (
    <LibraryPage
      libraries={libraries}
      besides={<MusicFound found={found} />}
      besidesFound={foundAny(found)}
    />
  );
}
