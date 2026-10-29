// The AI section of the panel: the AI switch (FR-62), the API key (FR-60) and the consent status (FR-61).
// The key goes straight to the backend, which keeps it in the OS credential store. After saving,
// the interface only ever sees its last 4 characters (NFR-05).
import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AI_PLATFORM, AI_PLATFORM_NAME } from "./ai";
import type { KeyStatus, Settings } from "./types";

/** The saved key's status for the AI platform: undefined while loading, null if it can't be read. */
export function useApiKey() {
  const [status, setStatus] = useState<KeyStatus | null | undefined>(undefined);
  const [error, setError] = useState("");

  useEffect(() => {
    invoke<KeyStatus>("api_key_status", { platform: AI_PLATFORM })
      .then(setStatus)
      .catch((e) => {
        setStatus(null);
        setError(String(e));
      });
  }, []);

  const run = useCallback(async (command: string, args: Record<string, string>) => {
    try {
      setStatus(await invoke<KeyStatus>(command, { platform: AI_PLATFORM, ...args }));
      setError("");
      return true;
    } catch (e) {
      setError(String(e));
      return false;
    }
  }, []);

  return {
    status,
    error,
    hasKey: Boolean(status?.last4),
    save: (key: string) => run("save_api_key", { key }),
    remove: () => run("delete_api_key", {}),
  };
}

/** The Claude model name (#15). Saved when the field loses focus or on Enter; blank restores the default. */
function ModelField({ saved, onSave }: { saved: string; onSave: (model: string) => void }) {
  const [draft, setDraft] = useState(saved);
  useEffect(() => setDraft(saved), [saved]);

  function commit() {
    if (draft.trim() !== saved) onSave(draft.trim());
  }

  return (
    <label>
      Claude model
      <input
        type="text"
        autoComplete="off"
        spellCheck={false}
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => e.key === "Enter" && commit()}
      />
    </label>
  );
}

interface Props {
  settings: Settings | null;
  apiKey: ReturnType<typeof useApiKey>;
  onChange: (change: Partial<Settings>) => Promise<string | null>;
}

export function AiSettings({ settings, apiKey, onChange }: Props) {
  const [draft, setDraft] = useState("");
  const [replacing, setReplacing] = useState(false);
  const [saving, setSaving] = useState(false);
  const [problem, setProblem] = useState("");
  const enabled = settings?.aiEnabled ?? false;
  const last4 = apiKey.status?.last4;

  async function change(value: Partial<Settings>) {
    setProblem((await onChange(value)) ?? "");
  }

  async function saveKey() {
    setSaving(true);
    const ok = await apiKey.save(draft);
    setSaving(false);
    // The pasted key is dropped from memory whether or not it was saved.
    setDraft("");
    if (ok) setReplacing(false);
  }

  return (
    <fieldset className="ai-settings">
      <legend>AI</legend>
      <label className="check">
        <input
          type="checkbox"
          checked={enabled}
          disabled={!settings}
          onChange={(e) => change({ aiEnabled: e.target.checked })}
        />
        Use AI for Smart mode
      </label>

      {enabled && (
        <>
          {last4 && !replacing ? (
            <div className="key-status">
              <span>
                {AI_PLATFORM_NAME} API key ending in <code>{last4}</code>
              </span>
              <div className="row">
                <button type="button" onClick={() => setReplacing(true)}>
                  Replace
                </button>
                <button type="button" onClick={apiKey.remove}>
                  Remove
                </button>
              </div>
            </div>
          ) : (
            <form
              className="key-form"
              onSubmit={(e) => {
                e.preventDefault();
                saveKey();
              }}
            >
              <label>
                {AI_PLATFORM_NAME} API key
                <input
                  type="password"
                  autoComplete="off"
                  spellCheck={false}
                  value={draft}
                  onChange={(e) => setDraft(e.target.value)}
                  placeholder="sk-ant-…"
                  disabled={apiKey.status === undefined}
                />
              </label>
              <div className="row">
                <button type="submit" disabled={saving || !draft.trim()}>
                  {saving ? "Saving…" : "Save key"}
                </button>
                {replacing && (
                  <button
                    type="button"
                    onClick={() => {
                      setDraft("");
                      setReplacing(false);
                    }}
                  >
                    Cancel
                  </button>
                )}
              </div>
            </form>
          )}
          <p className="muted">
            Kept in your system&rsquo;s credential store, never in a file. It is sent only to Anthropic.
          </p>
          <ModelField saved={settings?.aiModel ?? ""} onSave={(aiModel) => change({ aiModel })} />
          {settings?.aiConsent && (
            <p className="muted">
              You agreed to the AI notice.{" "}
              <button type="button" className="link" onClick={() => change({ aiConsent: false })}>
                Withdraw
              </button>
            </p>
          )}
        </>
      )}
      {(apiKey.error || problem) && (
        <p className="error" role="alert">
          {apiKey.error || problem}
        </p>
      )}
    </fieldset>
  );
}
