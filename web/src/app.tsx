/*
 * The shell: who is there, and where each address leads.
 *
 * Nothing of the library is drawn before the server has said who is asking.
 * Everything behind this point belongs to somebody, so the account comes
 * first rather than beside it, and no screen has to wonder whether there is
 * one.
 */

import { useEffect, useRef } from "react";
import { useBranding } from "./player/logo";
import { wearTheLogo } from "./installing";
import { nameTheTab } from "./tab";
import { Route, Routes, useLocation } from "react-router-dom";
import { api } from "./api";
import { AttentionProvider } from "./attention";
import { AdministrationLine } from "./live";
import { Header } from "./components/header";
import { ScrollBar } from "./components/scrollbar";
import { Toasts } from "./components/toasts";
import { DebugJournal } from "./components/debug-journal";
import { HomePage } from "./pages/home";
import { LibraryPage } from "./pages/library";
import { SearchPage } from "./pages/search";
import { PersonPage } from "./pages/person";
import { useKeptPlaces } from "./scrolling";
import { WorkPage } from "./pages/work";
import { AdminLayout } from "./pages/admin/layout";
import { AdminOverview } from "./pages/admin/overview";
import { AdminLibraries } from "./pages/admin/libraries";
import { AdminMetadata } from "./pages/admin/metadata";
import { AdminPlayback } from "./pages/admin/playback";
import { AdminTranscoding } from "./pages/admin/transcoding";
import { AdminUsers } from "./pages/admin/users";
import { AdminDevices } from "./pages/admin/devices";
import { AdminSecurity } from "./pages/admin/security";
import { AdminTasks } from "./pages/admin/tasks";
import { AdminJournal } from "./pages/admin/journal";
import { AdminDiagnostics } from "./pages/admin/diagnostics";
import { AdminSettings } from "./pages/admin/settings";
import { MySettingsLayout } from "./pages/settings/layout";
import { MyProfile } from "./pages/settings/profile";
import { MyAppearance } from "./pages/settings/appearance";
import { MyHomePage } from "./pages/settings/home";
import { MyPlayback } from "./pages/settings/playback";
import { Door } from "./pages/door";
import { LibrariesContext, useWatchedLibraries } from "./libraries";
import { RunningContext, useWatchedWork } from "./running";
import { useWhoIsThere, WhoProvider } from "./account";
import { MarksProvider } from "./marks";
import { useSettings } from "./settings";

export function App() {
  const who = useWhoIsThere();
  const branding = useBranding();
  useEffect(() => nameTheTab(branding?.server_name ?? null), [branding]);
  useEffect(() => wearTheLogo(branding?.logo_icon ?? null), [branding]);

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

  /* The box the whole library scrolls in, held so the bar drawn over it can
     read where it stands. */
  const scrolling = useRef<HTMLDivElement>(null);
  const location = useLocation();
  // Going back finds every page where it was left.
  useKeptPlaces(scrolling);

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
        <Toasts>
        <AdministrationLine>
        <AttentionProvider>
        {/* The bar stands over the page rather than beside it, so the page
            can be scrolled up behind it and read faintly through the glass.
            Where the two sit in the markup does not decide that on its own;
            it is the bar's own fixed position, in app.css, that lifts it out
            of the page. */}
        {/* DEBUG ONLY, TO BE REMOVED with components/debug-journal.tsx. */}
        <DebugJournal />
        <div className="shell">
          <Header libraries={libraries.all} scrolling={scrolling} />
          <div className="shell-scroll" ref={scrolling}>
            <Routes>
              <Route path="/" element={<HomePage libraries={libraries.all} />} />
              <Route path="/library/:id" element={<LibraryPage libraries={libraries.all} />} />
              <Route path="/search" element={<SearchPage libraries={libraries.all} />} />
              {/* The same grid, narrowed to what this account marked: a view of the
                  library rather than a library of its own. */}
              <Route path="/favourites" element={<LibraryPage libraries={libraries.all} />} />
              <Route path="/work/:id" element={<WorkPage />} />
              <Route path="/person/:id" element={<PersonPage />} />
              <Route path="/admin" element={<AdminLayout />}>
                <Route index element={<AdminOverview />} />
                <Route path="libraries" element={<AdminLibraries />} />
                <Route path="metadata" element={<AdminMetadata />} />
                <Route path="playback" element={<AdminPlayback />} />
                <Route path="transcoding" element={<AdminTranscoding />} />
                <Route path="users" element={<AdminUsers />} />
                <Route path="devices" element={<AdminDevices />} />
                <Route path="security" element={<AdminSecurity />} />
                <Route path="tasks" element={<AdminTasks />} />
                <Route path="journal" element={<AdminJournal />} />
                <Route path="diagnostics" element={<AdminDiagnostics />} />
                <Route path="settings" element={<AdminSettings />} />
              </Route>
              <Route path="/settings" element={<MySettingsLayout />}>
                <Route index element={<MyProfile />} />
                <Route path="appearance" element={<MyAppearance />} />
                <Route path="home" element={<MyHomePage />} />
                <Route path="playback" element={<MyPlayback />} />
              </Route>
              <Route path="*" element={<main className="page"><p className="notice">{t("error.not_found")}</p></main>} />
            </Routes>
            {/* Said once, on the home page, where the provider's pictures and
                words are shown first. Not under a grid, which is read to its
                end to find a film and where a line of small print only
                stands in the way, nor on the tools and somebody's own
                settings, which show none of them, nor on the page of a work
                or of a person, which is meant to be the film alone. */}
            {location.pathname === "/" && (
              <footer className="footer">
                <span>{t("attribution.tmdb")}</span>
              </footer>
            )}
          </div>

          {/* Outside the box it belongs to, because a bar drawn inside it
              would be cut off at the same edge everything else is. */}
          <ScrollBar holder={scrolling} />
        </div>
        </AttentionProvider>
        </AdministrationLine>
        </Toasts>
      </LibrariesContext.Provider>
    </RunningContext.Provider>
  );
}
