import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { formatSize, rejectReason } from "./file";
import type { PdfInfo, Preset, Range, RangeMode } from "./types";
import "./App.css";

const CUSTOM_PROMPT = "__custom__";

function App() {
  const [pdf, setPdf] = useState<PdfInfo | null>(null);
  const [dragging, setDragging] = useState(false);
  const [opening, setOpening] = useState(false);
  const [mode, setMode] = useState<RangeMode>("smart");
  const [presets, setPresets] = useState<Preset[]>([]);
  const [category, setCategory] = useState("");
  const [presetId, setPresetId] = useState("");
  const [customPrompt, setCustomPrompt] = useState("");
  const [customRanges, setCustomRanges] = useState("");
  const [fixedEvery, setFixedEvery] = useState(10);
  const [ranges, setRanges] = useState<Range[]>([]);
  const [error, setError] = useState("");

  useEffect(() => {
    invoke<Preset[]>("list_presets").then(setPresets).catch((e) => setError(String(e)));
  }, []);

  // FR-01, FR-02, FR-05: open one PDF and show its details, or say why it can't be opened.
  const openPaths = useCallback(async (paths: string[]) => {
    const reason = rejectReason(paths);
    if (reason) return setError(reason);
    if (paths.length === 0) return;
    setError("");
    setOpening(true);
    try {
      setPdf(await invoke<PdfInfo>("open_pdf", { path: paths[0] }));
      setRanges([]);
    } catch (e) {
      setError(String(e));
    } finally {
      setOpening(false);
    }
  }, []);

  const chooseFile = useCallback(async () => {
    const path = await open({ multiple: false, directory: false, filters: [{ name: "PDF", extensions: ["pdf"] }] });
    if (path) await openPaths([path]);
  }, [openPaths]);

  // FR-01: Tauri drag-drop events give real file paths, which HTML drag events don't.
  useEffect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent(({ payload }) => {
      if (payload.type === "enter" || payload.type === "over") setDragging(true);
      else if (payload.type === "leave") setDragging(false);
      else if (payload.type === "drop") {
        setDragging(false);
        openPaths(payload.paths);
      }
    });
    return () => {
      unlisten.then((stop) => stop());
    };
  }, [openPaths]);

  // NFR-11: Ctrl+O / Cmd+O opens the file picker.
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "o") {
        e.preventDefault();
        chooseFile();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [chooseFile]);

  const categories = useMemo(() => [...new Set(presets.map((p) => p.category))], [presets]);
  const categoryPresets = presets.filter((p) => p.category === category);

  // FR-23: the preset list follows the selected category.
  useEffect(() => {
    if (!category && categories.length) setCategory(categories[0]);
  }, [categories, category]);
  useEffect(() => {
    setPresetId(categoryPresets[0]?.id ?? CUSTOM_PROMPT);
  }, [category]); // eslint-disable-line react-hooks/exhaustive-deps

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
          <header className={dragging ? "file-info dragging" : "file-info"}>
            <div>
              <strong>{pdf.fileName}</strong>
              <span>
                {pdf.pageCount} {pdf.pageCount === 1 ? "page" : "pages"} · {formatSize(pdf.sizeBytes)}
              </span>
            </div>
            <button type="button" onClick={chooseFile} disabled={opening}>
              Open another
            </button>
          </header>
        ) : (
          <div className={dragging ? "dropzone dragging" : "dropzone"}>
            <p>{opening ? "Opening…" : "Drop a PDF here"}</p>
            <button type="button" onClick={chooseFile} disabled={opening} aria-keyshortcuts="Control+O Meta+O">
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
              <select value={presetId} onChange={(e) => setPresetId(e.target.value)}>
                {categoryPresets.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
                <option value={CUSTOM_PROMPT}>Custom prompt</option>
              </select>
            </label>
            {presetId === CUSTOM_PROMPT && (
              <label>
                Instruction
                <textarea
                  value={customPrompt}
                  onChange={(e) => setCustomPrompt(e.target.value)}
                  placeholder="Split whenever a new client name appears"
                />
              </label>
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
