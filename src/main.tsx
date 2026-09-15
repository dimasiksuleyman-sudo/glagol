import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import { getCurrentWindow } from "@tauri-apps/api/window";

import App from "./App";
import { OverlayPill } from "@/components/dictation/OverlayPill";
import { TtsProvider } from "@/contexts/TtsContext";
import { reportUiReady } from "@/lib/tauri";
import "./index.css";

// Both the main window and the dictation overlay load this same bundle
// (Sprint 6 PR3, D5). Branch on the window label so the tiny always-on-top
// pill never mounts the Router / AppShell / credentials machinery — it is a
// separate, self-contained view.
const isOverlay = getCurrentWindow().label === "overlay";
const root = ReactDOM.createRoot(document.getElementById("root") as HTMLElement);

if (isOverlay) {
  // The overlay window is transparent (D6); clear any page background the app
  // stylesheet sets so only the dark pill is visible.
  document.documentElement.style.background = "transparent";
  document.body.style.background = "transparent";
  root.render(
    <React.StrictMode>
      <OverlayPill />
    </React.StrictMode>,
  );
} else {
  root.render(
    <React.StrictMode>
      <TtsProvider>
        <BrowserRouter>
          <App />
        </BrowserRouter>
      </TtsProvider>
    </React.StrictMode>,
  );

  // Two frames place the marker after React's first visible main-window paint.
  // Rust de-duplicates the event for StrictMode and accidental repeated calls.
  window.requestAnimationFrame(() => {
    window.requestAnimationFrame(() => {
      void reportUiReady().catch(() => undefined);
    });
  });
}
