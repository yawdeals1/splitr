import React from "react";
import ReactDOM from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import type { Settings } from "./types";

// FR-63: load settings before the first render, so App shows the saved theme from its first paint
// and a forced light or dark theme doesn't flash the OS theme first. If settings can't be read, App
// tries again and shows why.
invoke<Settings>("get_settings")
  .catch(() => null)
  .then((settings) => {
    ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
      <React.StrictMode>
        <App initialSettings={settings} />
      </React.StrictMode>,
    );
  });
