import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { PresetEditor } from "./PresetEditor";
import type { PdfInfo, Preset, PresetInput, PresetList, Range, RangeMode } from "./types";
import "./App.css";

const CUSTOM_PROMPT = "__custom__";

function App() {
  const [pdf] = useState<PdfInfo | null>(null);
  const [mode, setMode] = useState<RangeMode>("smart");
  const [presets, setPresets] = useState<Preset[]>([]);
  const [category, setCategory] = useState("");
  const [presetId, setPresetId] = useState("");
  const [customPrompt, setCustomPrompt] = useState("");
  const [customRanges, setCustomRanges] = useState("");
  const [fixedEvery, setFixedEvery] = useState(10);
  const [ranges, setRanges] = useState<Range[]>([]);
  const [error, setError] = useState("");
  const [presetDraft, setPresetDraft] = useState<PresetInput | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const loadPresets = () =>
    invoke<PresetList>("list_presets")
      .then((list) => {
        setPresets(list.presets);
        if (list.problem) setError(list.problem);
      })
      .catch((e) => setError(String(e)));
  useEffect(() => {
    loadPresets();
  }, []);

  const categories = useMemo(() => [...new Set(presets.map((p) => p.category))], [presets]);
  const categoryPresets = presets.filter((p) => p.category === category);
  const selectedPreset = categoryPresets.find((p) => p.id === presetId);

  // FR-23: the preset list follows the selected category. A preset that's still in the list stays
  // selected. If the category is gone (its last user preset was deleted), pick the first one.
  useEffect(() => {
    if (categories.length && !categories.includes(category)) setCategory(categories[0]);
  }, [categories, category]);
  useEffect(() => {
    if (!categoryPresets.some((p) => p.id === presetId)) setPresetId(categoryPresets[0]?.id ?? CUSTOM_PROMPT);
    setPresetDraft(null);
    setConfirmDelete(false);
  }, [category, presets]); // eslint-disable-line react-hooks/exhaustive-deps

  // FR-71: after saving, show the saved preset selected in its category.
  async function presetSaved(preset: Preset) {
    await loadPresets();
    setCategory(preset.category);
    setPresetId(preset.id);
    setPresetDraft(null);
  }

  async function deletePreset(id: string) {
    try {
      await invoke("delete_user_preset", { id });
      setPresetId("");
      await loadPresets();
    } catch (e) {
      setError(String(e));
    }
    setConfirmDelete(false);
  }

  async function split() {
    setError("");
    if (!pdf) return setError("Open a PDF first.");
    try {
      if (mode === "custom") {
        setRanges(await invoke<Range[]>("parse_custom_ranges", { input: customRanges, pageCount: pdf.pageCount }));
      } else if (mode === "fixed") {
        setRanges(await invoke<Range[]>("fixed_ranges", { pageCount: pdf.pageCount, every: fixedEvery }));
      } else {
        setError("Smart split is not implemented yet.");
      }
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <main className="layout">
      <section className="canvas" aria-label="Document">
        {pdf ? (
          <p>
            {pdf.fileName} · {pdf.pageCount} pages
          </p>
        ) : (
          <div className="dropzone">
            <p>Drop a PDF here</p>
            <button type="button" disabled title="Coming in Phase 0">
              Choose file
            </button>
          </div>
        )}
        {ranges.length > 0 && (
          <ol className="parts">
            {ranges.map((r) => (
              <li key={r.start}>
                Pages {r.start}–{r.end} {r.title && `· ${r.title}`}
              </li>
            ))}
          </ol>
        )}
      </section>

      <aside className="panel" aria-label="Split options">
        <fieldset className="modes">
          <legend>Range mode</legend>
          {(["custom", "fixed", "smart"] as const).map((m) => (
            <label key={m} className={mode === m ? "active" : ""}>
              <input type="radio" name="mode" value={m} checked={mode === m} onChange={() => setMode(m)} />
              {m[0].toUpperCase() + m.slice(1)}
            </label>
          ))}
        </fieldset>

        {mode === "custom" && (
          <label>
            Page ranges
            <input value={customRanges} onChange={(e) => setCustomRanges(e.target.value)} placeholder="1-14, 15-30, 31" />
          </label>
        )}

        {mode === "fixed" && (
          <label>
            Split every N pages
            <input type="number" min={1} value={fixedEvery} onChange={(e) => setFixedEvery(Number(e.target.value))} />
          </label>
        )}

        {mode === "smart" && (
          <>
            <label>
              Document category
              <select value={category} onChange={(e) => setCategory(e.target.value)}>
                {categories.map((c) => (
                  <option key={c}>{c}</option>
                ))}
              </select>
            </label>
            <label>
              Split preset
              <select
                value={presetId}
                onChange={(e) => {
                  setPresetId(e.target.value);
                  setPresetDraft(null);
                  setConfirmDelete(false);
                }}
              >
                {categoryPresets.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
                <option value={CUSTOM_PROMPT}>Custom prompt</option>
              </select>
            </label>
            {presetDraft ? (
              <PresetEditor
                key={presetDraft.id ?? "new"}
                draft={presetDraft}
                categories={categories}
                onSaved={presetSaved}
                onCancel={() => setPresetDraft(null)}
              />
            ) : presetId === CUSTOM_PROMPT ? (
              <>
                <label>
                  Instruction
                  <textarea
                    value={customPrompt}
                    onChange={(e) => setCustomPrompt(e.target.value)}
                    placeholder="Split whenever a new client name appears"
                  />
                </label>
                <button
                  type="button"
                  disabled={!customPrompt.trim()}
                  onClick={() => setPresetDraft({ id: null, name: "", category: "My presets", prompt: customPrompt })}
                >
                  Save as preset
                </button>
              </>
            ) : (
              selectedPreset?.user && (
                <div className="row">
                  <button
                    type="button"
                    onClick={() =>
                      setPresetDraft({
                        id: selectedPreset.id,
                        name: selectedPreset.name,
                        category: selectedPreset.category,
                        prompt: selectedPreset.prompt,
                      })
                    }
                  >
                    Edit preset
                  </button>
                  {confirmDelete ? (
                    <button type="button" className="danger" autoFocus onClick={() => deletePreset(selectedPreset.id)}>
                      Confirm delete
                    </button>
                  ) : (
                    <button type="button" onClick={() => setConfirmDelete(true)}>
                      Delete preset
                    </button>
                  )}
                </div>
              )
            )}
          </>
        )}

        <button type="button" className="primary" onClick={split}>
          Split
        </button>
        {error && <p className="error" role="alert">{error}</p>}
      </aside>
    </main>
  );
}

export default App;
