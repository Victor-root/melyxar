/*
 * The shell: what is loaded once, and where each address leads.
 */

import { useEffect, useState } from "react";
import { Route, Routes } from "react-router-dom";
import { api } from "./api";
import type { Library } from "./api";
import { Header } from "./components/header";
import { HomePage } from "./pages/home";
import { LibraryPage } from "./pages/library";
import { SearchPage } from "./pages/search";
import { WorkPage } from "./pages/work";
import { ActivityPage } from "./pages/activity";
import { SettingsPage } from "./pages/settings";
import { RunningContext, useWatchedWork } from "./running";
import { useSettings } from "./settings";

export function App() {
  const { t } = useSettings();
  const [libraries, setLibraries] = useState<Library[]>([]);
  // Watched here, where the bar that starts the work and the pages that show
  // what it produced can both read it.
  const running = useWatchedWork();

  // The libraries are what the whole navigation is built from, so they are
  // fetched once here rather than by every page that mentions them.
  useEffect(() => {
    const controller = new AbortController();
    api
      .libraries(controller.signal)
      .then(setLibraries)
      .catch(() => setLibraries([]));
    return () => controller.abort();
  }, []);

  return (
    <RunningContext.Provider value={running}>
      <Header libraries={libraries} />
      <Routes>
        <Route path="/" element={<HomePage libraries={libraries} />} />
        <Route path="/library/:id" element={<LibraryPage libraries={libraries} />} />
        <Route path="/search" element={<SearchPage libraries={libraries} />} />
        <Route path="/work/:id" element={<WorkPage />} />
        <Route path="/activity" element={<ActivityPage libraries={libraries} />} />
        <Route path="/settings" element={<SettingsPage />} />
        <Route path="*" element={<main className="page"><p className="notice">{t("error.not_found")}</p></main>} />
      </Routes>
      <footer className="footer">
        <span>{t("attribution.tmdb")}</span>
      </footer>
    </RunningContext.Provider>
  );
}
