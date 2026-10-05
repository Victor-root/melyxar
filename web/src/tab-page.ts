/*
 * Names the tab after the page that is open, for as long as it is open.
 * Given nothing, a page leaves the tab as it is, which is how a page that
 * only sometimes has a name of its own, or that is named by one it holds,
 * stays out of the way.
 */

import { useEffect } from "react";
import { showPage } from "./tab";

export function useTabPage(name: string | null | undefined): void {
  useEffect(() => {
    if (!name) {
      return;
    }
    showPage(name);
    return () => showPage(null);
  }, [name]);
}
