/*
 * Searching, which is the library grid with the query in the address.
 *
 * Reusing the grid rather than building a second one means a search result
 * sorts, filters and pages exactly like everything else.
 */

import { LibraryPage } from "./library";
import type { Library } from "../api";

export function SearchPage({ libraries }: { libraries: Library[] }) {
  return <LibraryPage libraries={libraries} />;
}
