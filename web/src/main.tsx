import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import { App } from "./app";
import { WindowBackdrop } from "./components/backdrop";
import { GoToProvider } from "./navigating";
import { setUpMeasuring } from "./measure";
import { markTheEdgeRoom } from "./edge-room";
import { keepRingsForTheKeyboard } from "./keyboard-rings";
import { markLighter } from "./lighter";
import { SettingsProvider } from "./settings";
import "./styles/index.css";

// First, so that an armed recorder sees the page from its very start.
setUpMeasuring();
markTheEdgeRoom();
keepRingsForTheKeyboard();
markLighter();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <SettingsProvider>
      <WindowBackdrop />
      <BrowserRouter>
        <GoToProvider>
          <App />
        </GoToProvider>
      </BrowserRouter>
    </SettingsProvider>
  </StrictMode>,
);
