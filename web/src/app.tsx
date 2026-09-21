/*
 * The shell: who is there, and where each address leads.
 *
 * Nothing of the library is drawn before the server has said who is asking.
 * Everything behind this point belongs to somebody, so the account comes
 * first rather than beside it, and no screen has to wonder whether there is
 * one.
 */

import { useEffect } from "react";
import { Route, Routes } from "react-router-dom";
import { api } from "./api";
import { Header } from "./components/header";
import { HomePage } from "./pages/home";
import { LibraryPage } from "./pages/library";
import { SearchPage } from "./pages/search";
import { WorkPage } from "./pages/work";
import { ActivityPage } from "./pages/activity";
import { JournalPage } from "./pages/journal";
import { SettingsPage } from "./pages/settings";
import { Door } from "./pages/door";
import { LibrariesContext, useWatchedLibraries } from "./libraries";
import { RunningContext, useWatchedWork } from "./running";
import { useWhoIsThere, WhoProvider } from "./account";
import { MarksProvider } from "./marks";
import { useSettings } from "./settings";

export function App() {
  const who = useWhoIsThere();

  // A door that flashes up for a moment in front of somebody who is signed in
  // is worse than a moment of nothing, so nothing is drawn until the server
  // has answered.
  if (who.stillAsking || (!who.account && !who.branding)) {
    return <main className="page" aria-busy="true" />;
  }

  if (!who.account) {
    return <Door branding={who.branding!} cameIn={who.cameIn} />;
  }

  return (
    <WhoProvider who={who}>
      {/* What this viewer has said about a work, held above every screen: one
          press of a tick has to change every card showing that work, not the
          one that was pressed. */}
      <MarksProvider>
        <TheLibrary />
      </MarksProvider>
    </WhoProvider>
  );
}

/**
 * Everything behind the door.
 *
 * Its own component because what it holds is watched for as long as it is on
 * screen: a scan followed twice a second, the libraries read again when one
 * changes. None of that should be running while nobody is signed in, and a
 * component that is not drawn is a component that watches nothing.
 */
function TheLibrary() {
  const { t, adopt } = useSettings();
  // What this account chose, once there is an account to ask about. The
  // browser's own copy drew the door a moment ago; this is what carries a
  // choice from one machine to the next.
  useEffect(() => {
    const controller = new AbortController();
    api
      .preferences(controller.signal)
      .then(adopt)
      .catch(() => {
        // A preference that did not arrive leaves the browser's own copy in
        // place, which is what was already on the screen.
      });
    return () => controller.abort();
  }, [adopt]);

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
          {/* The same grid, narrowed to what this account marked: a view of the
              library rather than a library of its own. */}
          <Route path="/favourites" element={<LibraryPage libraries={libraries.all} />} />
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
