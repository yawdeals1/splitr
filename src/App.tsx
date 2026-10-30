import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { flushSync } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { aiBlock, aiBlockMessage, aiService, usesAi } from "./ai";
import { AiConsent } from "./AiConsent";
import { AiSettings, useApiKey } from "./AiSettings";
import { ExportOptions } from "./ExportOptions";
import { baseName, formatSize, rejectReason } from "./file";
import { DEFAULT_TEMPLATE, savedMessage, usableTemplate } from "./naming";
import { PageGrid } from "./PageGrid";
import { PageViewer } from "./PageViewer";
import { PasswordPrompt } from "./PasswordPrompt";
import { PresetEditor } from "./PresetEditor";
import { ReviewScreen } from "./ReviewScreen";
import { useSettings } from "./settings";
import { showTheme } from "./theme";
import {
  PASSWORD_REQUIRED,
  WRONG_PASSWORD,
  type ExportNames,
  type ExportRequest,
  type PdfInfo,
  type Preset,
  type PresetInput,
  type PresetList,
  type Range,
  type RangeMode,
  type SavedParts,
  type Settings,
  type Theme,
} from "./types";
import "./App.css";

const CUSTOM_PROMPT = "__custom__";
const THEMES: { value: Theme; label: string }[] = [
  { value: "system", label: "System" },
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
];

function App({ initialSettings }: { initialSettings: Settings | null }) {
  const [pdf, setPdf] = useState<PdfInfo | null>(null);
  // Counts successful opens, so reopening the same file shows its pages afresh.
  const [opened, setOpened] = useState(0);
  // FR-11: the page shown in the larger view.
  const [viewing, setViewing] = useState<number | null>(null);
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
  // FR-21: every N pages, or N equal parts.
  const [fixedBy, setFixedBy] = useState<"every" | "parts">("every");
  const [fixedCount, setFixedCount] = useState(10);
  const [ranges, setRanges] = useState<Range[]>([]);
  // Custom mode may leave pages out (FR-20); other modes must cover every page (FR-35).
  const [allowGaps, setAllowGaps] = useState(false);
  // Bumped on every split so the review screen starts fresh.
  const [reviewId, setReviewId] = useState(0);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [exporting, setExporting] = useState(false);
  // FR-51: the name template being edited; null shows the saved one. Saved with each export.
  const [template, setTemplate] = useState<string | null>(null);
  // FR-51: what the template makes of the current parts' titles, from the backend.
  const [names, setNames] = useState<ExportNames | null>(null);
  // FR-53: bundle the parts into one ZIP.
  const [zip, setZip] = useState(false);
  // FR-55: a file the last export saved, so "Open folder" can show it.
  const [savedFile, setSavedFile] = useState<string | null>(null);
  // FR-61: true while the AI notice is showing.
  const [askConsent, setAskConsent] = useState(false);
  // FR-03: the open PDF's password, kept in memory only, so export can read the file again.
  const password = useRef<string | undefined>(undefined);
  const [presetDraft, setPresetDraft] = useState<PresetInput | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  // True while a file dialog is showing or a PDF is opening; drops and Ctrl+O bypass the disabled buttons.
  const busy = useRef(false);
  // NFR-11: where focus goes when the control that had it disappears.
  const presetSelect = useRef<HTMLSelectElement>(null);
  const splitButton = useRef<HTMLButtonElement>(null);
  const openButton = useRef<HTMLButtonElement>(null);
  const chooseButton = useRef<HTMLButtonElement>(null);
  // Set when Escape cancels "Confirm delete", so the "Delete preset" button that replaces it takes focus.
  const refocusDelete = useRef(false);
  const { settings, loadError, update: updateSettings } = useSettings(initialSettings);
  const apiKey = useApiKey(settings?.aiPlatform);
  const theme = settings?.theme;
  const typedTemplate = template ?? settings?.nameTemplate ?? DEFAULT_TEMPLATE;
  // A blank field names files with the default, and the default is what gets saved.
  const nameTemplate = usableTemplate(typedTemplate);

  // FR-63: show the saved theme, in the page and (where the OS allows) the window title bar. This is
  // the only place that shows it. It runs before paint, and not before settings have loaded, so the
  // theme main.tsx loaded is never replaced by "system" for a moment.
  useLayoutEffect(() => {
    if (theme) showTheme(theme);
  }, [theme]);

  async function chooseTheme(value: Theme) {
    const problem = await updateSettings({ theme: value });
    if (problem) setError(`The theme could not be saved: ${problem}`);
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
  const openPath = useCallback(async (path: string, pw?: string) => {
    setError("");
    setNotice("");
    setSavedFile(null);
    setOpening(true);
    try {
      setPdf(await invoke<PdfInfo>("open_pdf", { path, password: pw }));
      password.current = pw;
      setOpened((n) => n + 1);
      setViewing(null);
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

  // FR-03: retry with the password. It goes only to the backend and is never saved.
  const unlock = useCallback(async (path: string, pw: string) => {
    if (busy.current) return;
    busy.current = true;
    try {
      await openPath(path, pw);
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

  // FR-51: the names follow the template and the part titles once typing pauses. A failed preview
  // only hides the names; export names the parts again and reports its own errors.
  const titles = JSON.stringify(ranges.map((r) => r.title));
  useEffect(() => {
    if (!pdf || ranges.length === 0) return setNames(null);
    let current = true;
    const timer = setTimeout(() => {
      exportNames(pdf.fileName, nameTemplate, ranges)
        .then((n) => current && setNames(n))
        .catch(() => current && setNames(null));
    }, PREVIEW_DELAY_MS);
    return () => {
      current = false;
      clearTimeout(timer);
    };
  }, [pdf, nameTemplate, titles]); // eslint-disable-line react-hooks/exhaustive-deps

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

  // NFR-11: when a file finishes opening, or fails to, focus that dropped to the page goes to the
  // open button. The control that had it may be gone (the review screen closes) or was disabled
  // while the file loaded. Focus that is still on a control (a drop while typing a range) stays.
  const wasOpening = useRef(false);
  useEffect(() => {
    const focusLost = !document.activeElement || document.activeElement === document.body;
    if (wasOpening.current && !opening && focusLost) (openButton.current ?? chooseButton.current)?.focus();
    wasOpening.current = opening;
  }, [opening]);

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

  // `consented` is true right after the user agrees, before the saved setting reaches `settings`.
  async function split(consented = false) {
    setError("");
    setNotice("");
    setSavedFile(null);
    if (!pdf) return setError("Open a PDF first.");
    let next: Range[];
    try {
      if (mode === "custom") {
        next = await invoke<Range[]>("parse_custom_ranges", { input: customRanges, pageCount: pdf.pageCount });
      } else if (mode === "fixed") {
        next = await invoke<Range[]>("fixed_ranges", { pageCount: pdf.pageCount, [fixedBy]: fixedCount });
      } else {
        // FR-60 to FR-62: AI runs only when it is on, a key is saved and the user agreed to the notice.
        if (usesAi(presetId === CUSTOM_PROMPT ? null : selectedPreset ?? null)) {
          if (!settings) throw "Settings are still loading.";
          const block = aiBlock({ ...settings, aiConsent: settings.aiConsent || consented }, apiKey.hasKey);
          if (block === "consent") return setAskConsent(true);
          if (block) throw aiBlockMessage(block, aiService(settings.aiPlatform));
        }
        throw "Smart split is not implemented yet.";
      }
    } catch (e) {
      // The parts of an earlier split no longer match the options, so they aren't kept.
      setRanges([]);
      return setError(String(e));
    }
    // The gap rule and a fresh review screen only change once the new parts have arrived.
    setAllowGaps(mode === "custom");
    setReviewId((n) => n + 1);
    setRanges(next);
  }

  async function consentGiven() {
    setAskConsent(false);
    const problem = await updateSettings({ aiConsent: true });
    if (problem) return setError(`Your answer could not be saved: ${problem}`);
    await split(true);
  }

  // FR-50 to FR-53: ask for a folder, then save one PDF per part there, or one ZIP of them all.
  async function saveParts(file: PdfInfo, parts: Range[]) {
    if (busy.current) return;
    busy.current = true;
    setError("");
    setNotice("");
    setSavedFile(null);
    try {
      const folder = await open({
        directory: true,
        multiple: false,
        title: "Choose a folder for the parts",
        defaultPath: settings?.lastOutputFolder ?? undefined,
      });
      if (!folder) return;
      setExporting(true);
      // Named afresh, so a preview still on its way can't give the files old names.
      const fileNames = await exportNames(file.fileName, nameTemplate, parts);
      const request: ExportRequest = { path: file.path, password: password.current, ranges: parts, folder, names: fileNames.parts };
      if (zip) {
        const saved = await invoke<string>("export_zip", { request, zipName: fileNames.zip });
        setNotice(savedMessage(parts.length, folder, 0, saved));
        setSavedFile(saved);
      } else {
        const saved = await invoke<SavedParts>("export_parts", { request });
        setNotice(savedMessage(saved.paths.length, folder, saved.renamed));
        setSavedFile(saved.paths[0] ?? null);
      }
      // Remembering the folder and template is a convenience, so a failure here isn't shown.
      await updateSettings({ lastOutputFolder: folder, nameTemplate });
    } catch (e) {
      setError(String(e));
    } finally {
      setExporting(false);
      busy.current = false;
    }
  }

  // FR-55: show the saved files in the system file manager.
  async function openFolder(path: string) {
    try {
      await revealItemInDir(path);
    } catch (e) {
      setError(`The folder could not be opened: ${e}`);
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
          <>
            <header className={dragging ? "file-info dragging" : "file-info"}>
              <div>
                <strong>{pdf.fileName}</strong>
                <span>
                  {pdf.pageCount} {pdf.pageCount === 1 ? "page" : "pages"} · {formatSize(pdf.sizeBytes)}
                </span>
              </div>
              {/* Takes focus when this header appears (autoFocus: the first file opens, or a password
                  prompt closes) and when focus was lost while a file loaded (the effect on `opening`),
                  so focus never drops to the page (NFR-11). */}
              <button
                type="button"
                ref={openButton}
                onClick={chooseFile}
                disabled={opening}
                autoFocus
                aria-keyshortcuts="Control+O Meta+O"
              >
                Open another
              </button>
            </header>
            <PageGrid key={opened} path={pdf.path} pageCount={pdf.pageCount} ranges={ranges} onOpen={setViewing} />
            {viewing !== null && (
              <PageViewer
                key={opened}
                path={pdf.path}
                page={viewing}
                pageCount={pdf.pageCount}
                onPage={setViewing}
                onClose={() => setViewing(null)}
              />
            )}
          </>
        ) : (
          <div className={dragging ? "dropzone dragging" : "dropzone"}>
            <p>{opening ? "Opening…" : "Drop a PDF here"}</p>
            <button
              type="button"
              ref={chooseButton}
              onClick={chooseFile}
              disabled={opening}
              autoFocus
              aria-keyshortcuts="Control+O Meta+O"
            >
              Choose file
            </button>
          </div>
        )}
        {pdf && ranges.length > 0 && (
          // FR-42: export is only reachable from the review screen.
          <ReviewScreen
            key={reviewId}
            totalPages={pdf.pageCount}
            ranges={ranges}
            allowGaps={allowGaps}
            onChange={setRanges}
            onBack={() => {
              // Commit the closed review screen first, so nothing that mounts with it can take focus back.
              flushSync(() => setRanges([]));
              splitButton.current?.focus();
            }}
            exporting={exporting}
            onExport={() => saveParts(pdf, ranges)}
          >
            <ExportOptions
              template={typedTemplate}
              onTemplate={setTemplate}
              names={names?.parts ?? []}
              zip={zip}
              onZip={setZip}
              zipName={names?.zip ?? ""}
            />
          </ReviewScreen>
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
          <>
            <fieldset className="modes fixed-by">
              <legend>Split</legend>
              {(
                [
                  ["every", "Every N pages"],
                  ["parts", "Into N equal parts"],
                ] as const
              ).map(([value, label]) => (
                <label key={value} className={fixedBy === value ? "active" : ""}>
                  <input
                    type="radio"
                    name="fixed-by"
                    value={value}
                    checked={fixedBy === value}
                    onChange={() => setFixedBy(value)}
                  />
                  {label}
                </label>
              ))}
            </fieldset>
            <label>
              {fixedBy === "every" ? "Pages per part" : "Number of parts"}
              <input
                type="number"
                min={1}
                max={fixedBy === "parts" ? pdf?.pageCount : undefined}
                value={fixedCount}
                onChange={(e) => setFixedCount(Number(e.target.value))}
                onKeyDown={splitOnEnter}
              />
            </label>
          </>
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
                      onKeyDown={(e) => {
                        if (e.key !== "Escape") return;
                        refocusDelete.current = true;
                        setConfirmDelete(false);
                      }}
                    >
                      Confirm delete
                    </button>
                  ) : (
                    <button
                      type="button"
                      ref={(button) => {
                        // NFR-11: focus moves back here when Escape removes "Confirm delete".
                        if (button && refocusDelete.current) {
                          refocusDelete.current = false;
                          button.focus();
                        }
                      }}
                      onClick={() => setConfirmDelete(true)}
                    >
                      Delete preset
                    </button>
                  )}
                </div>
              )
            )}
            <AiSettings settings={settings} apiKey={apiKey} onChange={updateSettings} />
          </>
        )}

        <button type="button" className="primary" onClick={() => split()} ref={splitButton}>
          Split
        </button>
        {error && <p className="error" role="alert">{error}</p>}
        {notice && (
          <div className="notice" role="status">
            <p>{notice}</p>
            {savedFile && (
              <button type="button" onClick={() => openFolder(savedFile)}>
                Open folder
              </button>
            )}
          </div>
        )}
        {askConsent && <AiConsent service={aiService(settings?.aiPlatform)} onAgree={consentGiven} onCancel={() => setAskConsent(false)} />}

        <fieldset className="modes theme">
          <legend>Theme</legend>
          {THEMES.map(({ value, label }) => (
            <label key={value} className={(theme ?? "system") === value ? "active" : ""}>
              <input
                type="radio"
                name="theme"
                value={value}
                checked={(theme ?? "system") === value}
                disabled={!settings}
                onChange={() => chooseTheme(value)}
              />
              {label}
            </label>
          ))}
          {loadError && (
            <p className="error" role="alert">
              Settings could not be loaded, so the theme can't be changed: {loadError}
            </p>
          )}
        </fieldset>
      </aside>
    </main>
  );
}

export default App;

/** How long typing must pause before the file names are previewed again. */
const PREVIEW_DELAY_MS = 200;

/** FR-51 to FR-53: the file names the template gives these parts, and the ZIP's, from the backend. */
function exportNames(fileName: string, template: string, parts: Range[]): Promise<ExportNames> {
  return invoke<ExportNames>("export_names", { template, original: fileName, titles: parts.map((r) => r.title) });
}
