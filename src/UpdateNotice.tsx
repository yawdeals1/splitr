// Offers a new version when one is published (FR-64). The check runs once at start; it is the only
// network request the app makes without an AI split (NFR-06).
import { useEffect, useState } from "react";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { downloadStep, updateMessage, type UpdateState } from "./update";

export function UpdateNotice() {
  const [update, setUpdate] = useState<Update | null>(null);
  const [state, setState] = useState<UpdateState | null>(null);

  useEffect(() => {
    let current = true;
    // Offline, or no release published yet: there is simply nothing to offer.
    check()
      .then((found) => {
        if (current && found) {
          setUpdate(found);
          setState({ kind: "available", version: found.version });
        }
      })
      .catch(() => {});
    return () => {
      current = false;
    };
  }, []);

  if (!update || !state) return null;

  async function install(found: Update) {
    try {
      await found.downloadAndInstall((event) => setState((s) => s && downloadStep(s, event)));
      await relaunch();
    } catch (e) {
      setState({ kind: "failed", message: String(e) });
    }
  }

  const busy = state.kind === "downloading" || state.kind === "restarting";
  return (
    <div className="notice" role="status">
      <p>{updateMessage(state)}</p>
      {!busy && (
        <div className="row">
          <button type="button" onClick={() => install(update)}>
            {state.kind === "failed" ? "Try again" : "Install and restart"}
          </button>
          <button type="button" onClick={() => setUpdate(null)}>
            Later
          </button>
        </div>
      )}
    </div>
  );
}
