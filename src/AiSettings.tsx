// The AI section of the panel: the AI switch (FR-62), the AI service and its model, the saved API
// keys (FR-60, #27) and the consent status (FR-61).
// A pasted key goes straight to the backend, which keeps it in the OS credential store. After
// saving, the interface only ever sees its last 4 characters (NFR-05).
import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AI_SERVICES, aiService, keyChoices, type KeyChoices, usesKey } from "./ai";
import type { KeyCheck, KeyDetection, KeyStatus, Settings } from "./types";

/** The platforms with a saved key (#27): undefined while loading. */
export function useApiKeys() {
  const [keys, setKeys] = useState<KeyStatus[] | undefined>(undefined);
  const [error, setError] = useState("");

  const refresh = useCallback(async () => {
    try {
      setKeys(await invoke<KeyStatus[]>("saved_api_keys"));
      setError("");
    } catch (e) {
      setKeys([]);
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const run = useCallback(
    async (command: string, args: Record<string, string>) => {
      try {
        await invoke<KeyStatus>(command, args);
        await refresh();
        return true;
      } catch (e) {
        setError(String(e));
        return false;
      }
    },
    [refresh],
  );

  return {
    keys,
    error,
    has: (platform: string | undefined) => Boolean(keys?.some((k) => k.platform === platform)),
    save: (platform: string, key: string) => run("save_api_key", { platform, key }),
    remove: (platform: string) => run("delete_api_key", { platform }),
  };
}

/**
 * A text setting saved when the field loses focus or on Enter. For the model, blank means the
 * default; `models` offers the platform's model list as suggestions (#30).
 */
function SavedField({
  label,
  saved,
  placeholder,
  onSave,
  models = [],
}: {
  label: string;
  saved: string;
  placeholder: string;
  onSave: (value: string) => void;
  models?: string[];
}) {
  const [draft, setDraft] = useState(saved);
  useEffect(() => setDraft(saved), [saved]);
  const listId = models.length > 0 ? `${label.toLowerCase().replace(/\W+/g, "-")}-options` : undefined;

  function commit() {
    if (draft.trim() !== saved) onSave(draft.trim());
  }

  return (
    <label>
      {label}
      <input
        type="text"
        placeholder={placeholder}
        autoComplete="off"
        spellCheck={false}
        value={draft}
        list={listId}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => e.key === "Enter" && commit()}
      />
      {listId && (
        <datalist id={listId}>
          {models.map((m) => (
            <option key={m} value={m} />
          ))}
        </datalist>
      )}
    </label>
  );
}

/** What the last check of a saved key found (#27). */
type CheckState = { state: "checking" } | { state: "connected" } | { state: "note" | "error"; text: string };

/**
 * #27: paste a key, see which platform it belongs to (detected locally from its prefix), correct
 * it if needed, and save. Nothing is sent anywhere until the key is saved and checked.
 */
function AddKey({ onSave }: { onSave: (platform: string, key: string) => Promise<boolean> }) {
  const [draft, setDraft] = useState("");
  const [choices, setChoices] = useState<KeyChoices | null>(null);
  const [platform, setPlatform] = useState("");
  const [saving, setSaving] = useState(false);

  function reset() {
    // The pasted key is dropped from memory whether or not it was saved.
    setDraft("");
    setChoices(null);
    setPlatform("");
  }

  async function detect() {
    const found = keyChoices(await invoke<KeyDetection>("detect_api_key", { key: draft }));
    setChoices(found);
    setPlatform(found.platform);
  }

  async function save() {
    setSaving(true);
    const ok = await onSave(platform, draft);
    setSaving(false);
    if (ok) reset();
  }

  return (
    <form
      className="key-form"
      onSubmit={(e) => {
        e.preventDefault();
        if (choices?.options.length) save();
        else detect();
      }}
    >
      <label>
        Add an API key
        <input
          type="password"
          autoComplete="off"
          spellCheck={false}
          value={draft}
          onChange={(e) => {
            setDraft(e.target.value);
            setChoices(null);
          }}
          placeholder="Paste a key"
        />
      </label>
      {choices?.problem && (
        <p className="error" role="alert">
          {choices.problem}
        </p>
      )}
      {choices && choices.options.length > 0 && (
        <label>
          {choices.detected ? "Detected platform. Change it if it's wrong" : "Which platform is this key for?"}
          <select value={platform} onChange={(e) => setPlatform(e.target.value)}>
            {!platform && (
              <option value="" disabled>
                Choose a platform
              </option>
            )}
            {choices.options.map((id) => (
              <option key={id} value={id}>
                {aiService(id).name}
              </option>
            ))}
          </select>
        </label>
      )}
      <div className="row">
        <button type="submit" disabled={saving || !draft.trim() || (choices !== null && !platform)}>
          {saving ? "Saving…" : !choices?.options.length ? "Next" : platform ? `Save ${aiService(platform).name} key` : "Save key"}
        </button>
        {draft && (
          <button type="button" onClick={reset}>
            Cancel
          </button>
        )}
      </div>
      {choices?.options.length && platform ? (
        <p className="muted">
          Kept in your system&rsquo;s credential store, never in a file. It is sent only to {aiService(platform).company},
          to check it and to split.
        </p>
      ) : null}
    </form>
  );
}

interface Props {
  settings: Settings | null;
  apiKeys: ReturnType<typeof useApiKeys>;
  onChange: (change: Partial<Settings>) => Promise<string | null>;
}

export function AiSettings({ settings, apiKeys, onChange }: Props) {
  const [problem, setProblem] = useState("");
  const [defaults, setDefaults] = useState<Record<string, string>>({});
  // #30: the chosen platform's models, fetched when the user asks.
  const [models, setModels] = useState<{ service: string; list: string[] } | null>(null);
  const [loadingModels, setLoadingModels] = useState(false);
  // #27: the result of the last check of each saved key, by platform.
  const [checks, setChecks] = useState<Record<string, CheckState>>({});
  const enabled = settings?.aiEnabled ?? false;
  const service = settings?.aiPlatform ?? "chatgpt";
  const current = aiService(service);

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

  // #30: asks the platform for its models, with its saved key. Only on request: it's a network call.
  async function loadModels() {
    setLoadingModels(true);
    try {
      const list = await invoke<string[]>("list_ai_models", { platform: service });
      setModels({ service, list });
      setProblem(list.length ? "" : `${current.name} didn't list any models. Type the model name.`);
    } catch (e) {
      setProblem(String(e));
    } finally {
      setLoadingModels(false);
    }
  }

  // #27: one call to the key's own platform, to show Connected or the exact error.
  async function check(platform: string) {
    setChecks((c) => ({ ...c, [platform]: { state: "checking" } }));
    let result: CheckState;
    try {
      const found = await invoke<KeyCheck>("verify_api_key", { platform });
      result = found.status === "connected" ? { state: "connected" } : { state: "note", text: found.reason };
    } catch (e) {
      result = { state: "error", text: String(e) };
    }
    setChecks((c) => ({ ...c, [platform]: result }));
  }

  async function addKey(platform: string, key: string) {
    const ok = await apiKeys.save(platform, key);
    if (!ok) return false;
    // A new key becomes the service Smart mode uses if the current one can't run yet.
    if (!usesKey(current) || !apiKeys.has(service)) await change({ aiPlatform: platform });
    check(platform);
    return true;
  }

  async function removeKey(platform: string) {
    if (await apiKeys.remove(platform)) setChecks(({ [platform]: _, ...rest }) => rest);
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
                {usesKey(s) && apiKeys.has(s.id) ? " (key saved)" : ""}
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

      {enabled && usesKey(current) && apiKeys.keys && !apiKeys.has(service) && (
        <p className="muted">Add your {current.name} API key below to use it.</p>
      )}

      {enabled && service === "custom" && (
        <SavedField
          label="Address"
          saved={settings?.aiCustomUrl ?? ""}
          placeholder="https://example.com/v1"
          onSave={(url) => change({ aiCustomUrl: url })}
        />
      )}

      {enabled && (
        <>
          <SavedField
            key={service}
            label="Model"
            saved={settings?.aiModels[service] ?? ""}
            placeholder={defaults[service] ?? ""}
            models={models?.service === service ? models.list : []}
            onSave={(model) => change({ aiModels: { ...settings?.aiModels, [service]: model } })}
          />
          {current.listsModels && apiKeys.has(service) && (
            <button type="button" className="link" disabled={loadingModels} onClick={loadModels}>
              {loadingModels ? "Loading models…" : "Show available models"}
            </button>
          )}

          {apiKeys.keys && apiKeys.keys.length > 0 && (
            <ul className="key-list" aria-label="Saved API keys">
              {apiKeys.keys.map(({ platform, last4 }) => {
                const result = checks[platform];
                return (
                  <li key={platform}>
                    <span>
                      {aiService(platform).name} key ending in <code>{last4}</code>
                    </span>
                    <div className="row">
                      <button type="button" onClick={() => check(platform)} disabled={result?.state === "checking"}>
                        {result?.state === "checking" ? "Checking…" : "Check"}
                      </button>
                      <button type="button" onClick={() => removeKey(platform)}>
                        Remove
                      </button>
                    </div>
                    {result?.state === "connected" && <p className="notice">Connected</p>}
                    {result?.state === "note" && <p className="muted">{result.text}</p>}
                    {result?.state === "error" && (
                      <p className="error" role="alert">
                        {result.text}
                      </p>
                    )}
                  </li>
                );
              })}
            </ul>
          )}
          <AddKey onSave={addKey} />

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
      {(apiKeys.error || problem) && (
        <p className="error" role="alert">
          {apiKeys.error || problem}
        </p>
      )}
    </fieldset>
  );
}
