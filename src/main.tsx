import React from "react";
import ReactDOM from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import { applyTheme } from "./theme";
import type { Settings } from "./types";

// FR-63: apply the saved theme before the first render, so a forced light or dark theme doesn't
// flash the OS theme first. If settings can't be read, App shows why.
invoke<Settings>("get_settings")
  .then((s) => applyTheme(document.documentElement, s.theme))
  .catch(() => {})
  .finally(() => {
    ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
      <React.StrictMode>
        <App />
      </React.StrictMode>,
    );
  });
