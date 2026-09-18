/*
 * The shell: what is loaded once, and where each address leads.
 */

import { Route, Routes } from "react-router-dom";
import { Header } from "./components/header";
import { HomePage } from "./pages/home";
import { LibraryPage } from "./pages/library";
import { SearchPage } from "./pages/search";
import { WorkPage } from "./pages/work";
import { ActivityPage } from "./pages/activity";
import { JournalPage } from "./pages/journal";
import { SettingsPage } from "./pages/settings";
import { LibrariesContext, useWatchedLibraries } from "./libraries";
import { RunningContext, useWatchedWork } from "./running";
import { useSettings } from "./settings";

export function App() {
  const { t } = useSettings();
  // Watched here, where the bar that starts the work and the pages that show
  // what it produced can both read it.
  const running = useWatchedWork();
  // The libraries are what the whole navigation is built from, so they are
  // held here rather than by every page that mentions them, and read again
  // whenever the settings screen changes one or a scan ends.
  const libraries = useWatchedLibraries(running.finished);

  return (
    <RunningContext.Provider value={running}>
      <LibrariesContext.Provider value={libraries}>
        <Header libraries={libraries.all} />
        <Routes>
          <Route path="/" element={<HomePage libraries={libraries.all} />} />
          <Route path="/library/:id" element={<LibraryPage libraries={libraries.all} />} />
          <Route path="/search" element={<SearchPage libraries={libraries.all} />} />
          <Route path="/work/:id" element={<WorkPage />} />
          <Route path="/activity" element={<ActivityPage libraries={libraries.all} />} />
          <Route path="/journal" element={<JournalPage />} />
          <Route path="/settings" element={<SettingsPage />} />
          <Route path="*" element={<main className="page"><p className="notice">{t("error.not_found")}</p></main>} />
        </Routes>
        <footer className="footer">
          <span>{t("attribution.tmdb")}</span>
        </footer>
      </LibrariesContext.Provider>
    </RunningContext.Provider>
  );
}
