// Loads and saves user settings through the backend (SRS 6.4).
import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "./types";

/**
 * Current settings (null until loaded, or if loading failed), the load error if any, a function
 * that saves a change and returns any error, and one that returns what was last saved, which is
 * newer than `settings` while a render is pending.
 */
export function useSettings() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  // The latest settings and the save in progress. Each change is merged into what the previous
  // save stored, so changes made close together can't overwrite each other.
  const latest = useRef<Settings | null>(null);
  const queue = useRef<Promise<unknown>>(Promise.resolve());

  useEffect(() => {
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

  const saved = useCallback(() => latest.current, []);

  return { settings, loadError, update, saved };
}
