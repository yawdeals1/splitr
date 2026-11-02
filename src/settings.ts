// Loads and saves user settings through the backend (SRS 6.4).
import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "./types";

/**
 * Current settings (null until loaded, or if loading failed), the load error if any, and a function
 * that saves a change and returns any error. `initial` is settings already loaded at startup; when
 * it is null they are loaded here.
 */
export function useSettings(initial: Settings | null = null) {
  const [settings, setSettings] = useState<Settings | null>(initial);
  const [loadError, setLoadError] = useState<string | null>(null);
  // The latest settings and the save in progress. Each change is merged into what the previous
  // save stored, so changes made close together can't overwrite each other.
  const latest = useRef<Settings | null>(initial);
  const queue = useRef<Promise<unknown>>(Promise.resolve());

  useEffect(() => {
    if (latest.current) return;
    invoke<Settings>("get_settings")
      .then((s) => {
        latest.current = s;
        setSettings(s);
      })
      .catch((e) => setLoadError(String(e)));
  }, []);

  const update = useCallback((change: Partial<Settings>): Promise<string | null> => {
    const run = queue.current.then(async () => {
      if (!latest.current) return loadError ?? "Settings are still loading.";
      try {
        const saved = await invoke<Settings>("save_settings", { settings: { ...latest.current, ...change } });
        latest.current = saved;
        setSettings(saved);
        return null;
      } catch (e) {
        return String(e);
      }
    });
    queue.current = run;
    return run;
  }, [loadError]);

  return { settings, loadError, update };
}
