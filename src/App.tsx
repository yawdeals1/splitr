import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { baseName, formatSize, rejectReason } from "./file";
import { PasswordPrompt } from "./PasswordPrompt";
import { PresetEditor } from "./PresetEditor";
import { useSettings } from "./settings";
import { applyTheme } from "./theme";
import {
  PASSWORD_REQUIRED,
  WRONG_PASSWORD,
  type PdfInfo,
  type Preset,
  type PresetInput,
  type PresetList,
  type Range,
  type RangeMode,
  type Theme,
} from "./types";
import "./App.css";

const CUSTOM_PROMPT = "__custom__";
const THEMES: { value: Theme; label: string }[] = [
  { value: "system", label: "System" },
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
];

function App() {
  const [pdf, setPdf] = useState<PdfInfo | null>(null);
  const [dragging, setDragging] = useState(false);
  const [opening, setOpening] = useState(false);
  // FR-03: an encrypted file waiting for its password.
  const [locked, setLocked] = useState<{ path: string; message: string } | null>(null);
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
  // True while a file dialog is showing or a PDF is opening; drops and Ctrl+O bypass the disabled buttons.
  const busy = useRef(false);
  // NFR-11: where focus goes when the control that had it disappears.
  const presetSelect = useRef<HTMLSelectElement>(null);
  const { settings, update: updateSettings } = useSettings();
  const theme = settings?.theme ?? "system";

  // FR-63: show the saved theme, in the page and (where the OS allows) the window title bar.
  useEffect(() => {
    applyTheme(document.documentElement, theme);
    getCurrentWindow()
      .setTheme(theme === "system" ? null : theme)
      .catch(() => {});
  }, [theme]);

  async function chooseTheme(value: Theme) {
    applyTheme(document.documentElement, value);
    const problem = await updateSettings({ theme: value });
    if (problem) {
      applyTheme(document.documentElement, theme);
      setError(`The theme could not be saved: ${problem}`);
    }
  }

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

  // FR-01 to FR-05: open one PDF and show its details, ask for its password, or say why it can't be opened.
  const openPath = useCallback(async (path: string, password?: string) => {
    setError("");
    setOpening(true);
    try {
      setPdf(await invoke<PdfInfo>("open_pdf", { path, password }));
      setRanges([]);
      setLocked(null);
    } catch (e) {
      const message = String(e);
      if (message === PASSWORD_REQUIRED || message === WRONG_PASSWORD) setLocked({ path, message });
      else {
        setLocked(null);
        setError(message);
      }
    } finally {
      setOpening(false);
    }
  }, []);

  // FR-03: retry with the password. It goes only to open_pdf and isn't kept.
  const unlock = useCallback(async (path: string, password: string) => {
    if (busy.current) return;
    busy.current = true;
    try {
      await openPath(path, password);
    } finally {
      busy.current = false;
    }
  }, [openPath]);

  const openDropped = useCallback(async (paths: string[]) => {
    if (busy.current) return;
    const reason = rejectReason(paths);
    if (reason) return setError(reason);
    busy.current = true;
    try {
      await openPath(paths[0]);
    } finally {
      busy.current = false;
    }
  }, [openPath]);

  const chooseFile = useCallback(async () => {
    if (busy.current) return;
    busy.current = true;
    try {
      // Both letter cases: GTK file filters on Linux are case-sensitive.
      const path = await open({ multiple: false, directory: false, filters: [{ name: "PDF", extensions: ["pdf", "PDF"] }] });
      if (path) await openPath(path);
    } catch (e) {
      setError(`The file picker could not open: ${e}`);
    } finally {
      busy.current = false;
    }
  }, [openPath]);

  // FR-01: Tauri drag-drop events give real file paths, which HTML drag events don't.
  useEffect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent(({ payload }) => {
      if (payload.type === "enter" || payload.type === "over") setDragging(true);
      else if (payload.type === "leave") setDragging(false);
      else if (payload.type === "drop") {
        setDragging(false);
        openDropped(payload.paths);
      }
    });
    return () => {
      unlisten.then((stop) => stop());
    };
  }, [openDropped]);

  // NFR-11: Ctrl+O / Cmd+O opens the file picker.
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "o") {
        e.preventDefault();
        if (!e.repeat) chooseFile();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [chooseFile]);

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
    presetSelect.current?.focus();
  }

  function cancelPresetEdit() {
    setPresetDraft(null);
    presetSelect.current?.focus();
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
    presetSelect.current?.focus();
  }

  // NFR-11: Enter in a range field runs the split.
  function splitOnEnter(e: React.KeyboardEvent) {
    if (e.key === "Enter" && !e.nativeEvent.isComposing) {
      e.preventDefault();
      split();
    }
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
        {locked ? (
          <PasswordPrompt
            fileName={baseName(locked.path)}
            message={locked.message}
            busy={opening}
            onSubmit={(password) => unlock(locked.path, password)}
            onCancel={() => setLocked(null)}
          />
        ) : pdf ? (
          <header className={dragging ? "file-info dragging" : "file-info"}>
            <div>
              <strong>{pdf.fileName}</strong>
              <span>
                {pdf.pageCount} {pdf.pageCount === 1 ? "page" : "pages"} · {formatSize(pdf.sizeBytes)}
              </span>
            </div>
            {/* Takes focus when the file opens or the password prompt is cancelled (NFR-11). */}
            <button type="button" onClick={chooseFile} disabled={opening} autoFocus aria-keyshortcuts="Control+O Meta+O">
              Open another
            </button>
          </header>
        ) : (
          <div className={dragging ? "dropzone dragging" : "dropzone"}>
            <p>{opening ? "Opening…" : "Drop a PDF here"}</p>
            <button type="button" onClick={chooseFile} disabled={opening} autoFocus aria-keyshortcuts="Control+O Meta+O">
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
            <input
              value={customRanges}
              onChange={(e) => setCustomRanges(e.target.value)}
              onKeyDown={splitOnEnter}
              placeholder="1-14, 15-30, 31"
            />
          </label>
        )}

        {mode === "fixed" && (
          <label>
            Split every N pages
            <input
              type="number"
              min={1}
              value={fixedEvery}
              onChange={(e) => setFixedEvery(Number(e.target.value))}
              onKeyDown={splitOnEnter}
            />
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
                ref={presetSelect}
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
                onCancel={cancelPresetEdit}
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
                    <button
                      type="button"
                      className="danger"
                      autoFocus
                      onClick={() => deletePreset(selectedPreset.id)}
                      onKeyDown={(e) => e.key === "Escape" && setConfirmDelete(false)}
                    >
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

        <fieldset className="modes theme">
          <legend>Theme</legend>
          {THEMES.map(({ value, label }) => (
            <label key={value} className={theme === value ? "active" : ""}>
              <input
                type="radio"
                name="theme"
                value={value}
                checked={theme === value}
                disabled={!settings}
                onChange={() => chooseTheme(value)}
              />
              {label}
            </label>
          ))}
        </fieldset>
      </aside>
    </main>
  );
}

export default App;
