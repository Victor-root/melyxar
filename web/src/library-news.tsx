/*
 * What a library holds, read again the moment it changes.
 *
 * A film that comes in, is named or gets its picture changes what the home
 * page, the grids and the bar show. The server says so on the live line, and
 * whoever shows it reads it again, so nobody ever reloads a page to see what
 * the libraries hold now.
 *
 * And what this account made of its works, the same way: a film left on the
 * television stands where it was left in the row to carry on, on every
 * other screen of the account, the moment it is left.
 */

import { useLibraries } from "./libraries";
import { useLibrariesNews, useMarksNews } from "./live";
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

export function WhenMarksMove() {
  const { rowsHaveMoved } = useMarks();
  useMarksNews(rowsHaveMoved);
  return null;
}
