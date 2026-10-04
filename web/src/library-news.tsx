/*
 * What a library holds, read again the moment it changes.
 *
 * A film that comes in, is named or gets its picture changes what the home
 * page, the grids and the bar show. The server says so on the live line, and
 * whoever shows it reads it again, so nobody ever reloads a page to see what
 * the libraries hold now.
 */

import { useLibraries } from "./libraries";
import { useLibrariesNews } from "./live";
import { useMarks } from "./marks";

export function WhenLibrariesMove() {
  const { refresh } = useLibraries();
  const { rowsHaveMoved } = useMarks();
  useLibrariesNews(() => {
    refresh();
    rowsHaveMoved();
  });
  return null;
}
