import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { RouterProvider, createBrowserRouter } from "react-router-dom";
import { App } from "./app";
import { setUpMeasuring } from "./measure";
import { SettingsProvider } from "./settings";
import "./styles/index.css";

// First, so that an armed recorder sees the page from its very start.
setUpMeasuring();

/* Every address goes to the app, which lays out its own routes. A router
   made this way hands out a way to navigate that never changes, so what only
   navigates when pressed, a card or the menu of a song, is not drawn again
   each time the address does. */
const router = createBrowserRouter([{ path: "*", element: <App /> }]);

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <SettingsProvider>
      <RouterProvider router={router} />
    </SettingsProvider>
  </StrictMode>,
);
