// Loads and saves user settings through the backend (SRS 6.4).
import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "./types";

/** Current settings (null until loaded) and a function that saves a change and returns any error. */
export function useSettings() {
  const [settings, setSettings] = useState<Settings | null>(null);

  useEffect(() => {
    invoke<Settings>("get_settings").then(setSettings);
  }, []);

  const update = useCallback(
    async (change: Partial<Settings>): Promise<string | null> => {
      if (!settings) return "Settings are still loading.";
      try {
        setSettings(await invoke<Settings>("save_settings", { settings: { ...settings, ...change } }));
        return null;
      } catch (e) {
        return String(e);
      }
    },
    [settings],
  );

  return { settings, update };
}
