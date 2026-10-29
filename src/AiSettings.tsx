// The AI section of the panel: the AI switch (FR-62), the AI service and its model, the API key (FR-60)
// and the consent status (FR-61).
// The key goes straight to the backend, which keeps it in the OS credential store. After saving,
// the interface only ever sees its last 4 characters (NFR-05).
import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AI_SERVICES, aiService, usesKey } from "./ai";
import type { KeyStatus, Settings } from "./types";

/**
 * The saved key's status for `platform`: undefined while loading, null if it can't be read or the
 * service signs in instead of using a key.
 */
export function useApiKey(platform: string | undefined) {
  const [status, setStatus] = useState<KeyStatus | null | undefined>(undefined);
  const [error, setError] = useState("");
  const keyPlatform = usesKey(aiService(platform)) ? platform : undefined;

  useEffect(() => {
    setStatus(undefined);
    setError("");
    if (!keyPlatform) return setStatus(null);
    let current = true;
    invoke<KeyStatus>("api_key_status", { platform: keyPlatform })
      .then((s) => current && setStatus(s))
      .catch((e) => {
        if (!current) return;
        setStatus(null);
        setError(String(e));
      });
    return () => {
      current = false;
    };
  }, [keyPlatform]);

  const run = useCallback(
    async (command: string, args: Record<string, string>) => {
      if (!keyPlatform) return false;
      try {
        setStatus(await invoke<KeyStatus>(command, { platform: keyPlatform, ...args }));
        setError("");
        return true;
      } catch (e) {
        setError(String(e));
        return false;
      }
    },
    [keyPlatform],
  );

  return {
    status,
    error,
    hasKey: Boolean(status?.last4),
    save: (key: string) => run("save_api_key", { key }),
    remove: () => run("delete_api_key", {}),
  };
}

/** The model for the chosen service. Saved when the field loses focus or on Enter; blank means the default. */
function ModelField({ saved, fallback, onSave }: { saved: string; fallback: string; onSave: (model: string) => void }) {
  const [draft, setDraft] = useState(saved);
  useEffect(() => setDraft(saved), [saved]);

  function commit() {
    if (draft.trim() !== saved) onSave(draft.trim());
  }

  return (
    <label>
      Model
      <input
        type="text"
        placeholder={fallback}
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
  const [defaults, setDefaults] = useState<Record<string, string>>({});
  const enabled = settings?.aiEnabled ?? false;
  const service = settings?.aiPlatform ?? "chatgpt";
  const current = aiService(service);
  const last4 = apiKey.status?.last4;

  useEffect(() => {
    invoke<Record<string, string>>("default_ai_models").then(setDefaults, () => setDefaults({}));
  }, []);

  // FR-61: the notice names the company that gets the excerpts, so a new service needs a new answer.
  function chooseService(id: string) {
    if (id !== service) change({ aiPlatform: id, aiConsent: false });
  }

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
        <label>
          AI service
          <select value={service} disabled={!settings} onChange={(e) => chooseService(e.target.value)}>
            {AI_SERVICES.map((s) => (
              <option key={s.id} value={s.id}>
                {s.label}
              </option>
            ))}
          </select>
        </label>
      )}

      {enabled && service === "chatgpt" && (
        <p className="muted">
          Signing in with your ChatGPT account is coming. Until then, choose a service that uses an API key.
        </p>
      )}

      {enabled && usesKey(current) && (
        <>
          {last4 && !replacing ? (
            <div className="key-status">
              <span>
                {current.name} API key ending in <code>{last4}</code>
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
                {current.name} API key
                <input
                  type="password"
                  autoComplete="off"
                  spellCheck={false}
                  value={draft}
                  onChange={(e) => setDraft(e.target.value)}
                  placeholder={current.keyHint}
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
            Kept in your system&rsquo;s credential store, never in a file. It is sent only to {current.company}.
          </p>
        </>
      )}

      {enabled && (
        <>
          <ModelField
            key={service}
            saved={settings?.aiModels[service] ?? ""}
            fallback={defaults[service] ?? ""}
            onSave={(model) => change({ aiModels: { ...settings?.aiModels, [service]: model } })}
          />
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
