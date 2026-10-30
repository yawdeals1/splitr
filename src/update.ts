// Checking for and installing new versions (FR-64, SRS 8 step 5). The update manifest and every
// download are verified with the public key in tauri.conf.json before anything installs.

/** Where an update stands, for the notice in the panel. */
export type UpdateState =
  | { kind: "available"; version: string }
  | { kind: "downloading"; received: number; total: number | null }
  | { kind: "restarting" }
  | { kind: "failed"; message: string };

/** One event of the updater plugin's download progress. */
export type DownloadEvent =
  | { event: "Started"; data: { contentLength?: number } }
  | { event: "Progress"; data: { chunkLength: number } }
  | { event: "Finished" };

/** The download state after `event`. */
export function downloadStep(state: UpdateState, event: DownloadEvent): UpdateState {
  const received = state.kind === "downloading" ? state.received : 0;
  const total = state.kind === "downloading" ? state.total : null;
  switch (event.event) {
    case "Started":
      return { kind: "downloading", received: 0, total: event.data.contentLength ?? null };
    case "Progress":
      return { kind: "downloading", received: received + event.data.chunkLength, total };
    case "Finished":
      return { kind: "restarting" };
  }
}

/** The notice text for `state`. */
export function updateMessage(state: UpdateState): string {
  switch (state.kind) {
    case "available":
      return `Splitr ${state.version} is available.`;
    case "downloading": {
      if (!state.total) return "Downloading the update…";
      const percent = Math.min(100, Math.floor((state.received / state.total) * 100));
      return `Downloading the update: ${percent}%`;
    }
    case "restarting":
      return "Installing the update. Splitr will restart.";
    case "failed":
      return `The update could not be installed: ${state.message}`;
  }
}
