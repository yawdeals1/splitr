// Form to save a custom prompt as a preset, or edit a saved one (FR-71).
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Preset, PresetInput } from "./types";

interface Props {
  draft: PresetInput;
  categories: string[];
  onSaved: (preset: Preset) => void;
  onCancel: () => void;
}

export function PresetEditor({ draft, categories, onSaved, onCancel }: Props) {
  const [value, setValue] = useState(draft);
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const set = (change: Partial<PresetInput>) => setValue((v) => ({ ...v, ...change }));

  async function save(e: React.FormEvent) {
    e.preventDefault();
    setSaving(true);
    setError("");
    try {
      onSaved(await invoke<Preset>("save_user_preset", { preset: value }));
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  }

  return (
    <form
      className="preset-editor"
      aria-label={draft.id ? "Edit preset" : "Save as preset"}
      onSubmit={save}
      onKeyDown={(e) => e.key === "Escape" && onCancel()}
    >
      <label>
        Preset name
        <input autoFocus required value={value.name} onChange={(e) => set({ name: e.target.value })} />
      </label>
      <label>
        Category
        <input
          required
          list="preset-categories"
          value={value.category}
          onChange={(e) => set({ category: e.target.value })}
        />
        <datalist id="preset-categories">
          {categories.map((c) => (
            <option key={c} value={c} />
          ))}
        </datalist>
      </label>
      <label>
        Instruction
        <textarea required value={value.prompt} onChange={(e) => set({ prompt: e.target.value })} />
      </label>
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      <div className="row">
        <button type="submit" disabled={saving}>
          Save preset
        </button>
        <button type="button" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </form>
  );
}
