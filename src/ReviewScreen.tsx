// Review and edit the proposed parts before anything is written (FR-40 to FR-42).
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  canJoin,
  focusAfter,
  gaps,
  mergeWithNext,
  pageCount,
  parsePage,
  removePart,
  rename,
  setEnd,
  setStart,
  splitAt,
  uncoveredPages,
  within,
  type Span,
} from "./review";
import type { Range } from "./types";

interface Props {
  totalPages: number;
  ranges: Range[];
  /** True when pages may be left out from the start, as in Custom mode. */
  allowGaps: boolean;
  onChange: (ranges: Range[]) => void;
  onBack: () => void;
  onExport: () => void;
  /** True while the parts are being saved (FR-50). */
  exporting?: boolean;
}

export function ReviewScreen({ totalPages, ranges, allowGaps, onChange, onBack, onExport, exporting = false }: Props) {
  // Pages of parts the user deleted. Only these may be missing when the mode must cover every page.
  const [leftOut, setLeftOut] = useState<Span[]>([]);
  // Page fields whose text isn't a valid page yet, by field. Their parts keep the last valid page.
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  // The backend's verdict on the part bounds it last checked (FR-35).
  const [checked, setChecked] = useState<{ key: string; problem: string } | null>(null);
  const [splitting, setSplitting] = useState<{ index: number; page: string } | null>(null);
  const [splitError, setSplitError] = useState("");
  // NFR-11: the control to focus after the one that had focus is removed or disabled, by its
  // data-focus value. The heading takes focus when the screen opens.
  const screen = useRef<HTMLElement>(null);
  const [focusNext, setFocusNext] = useState<string | null>("heading");

  useEffect(() => {
    if (!focusNext) return;
    screen.current?.querySelector<HTMLElement>(`[data-focus="${focusNext}"]`)?.focus();
    setFocusNext(null);
  }, [focusNext]);

  const gapsOk = allowGaps || within(gaps(ranges, totalPages), leftOut, totalPages);
  // Titles don't change the verdict, so only the bounds and the gap rule are checked again.
  const key = JSON.stringify([ranges.map((r) => [r.start, r.end]), totalPages, gapsOk]);

  // FR-35: every change to the bounds is checked by the backend before export is allowed.
  useEffect(() => {
    let current = true;
    invoke("validate_ranges", { ranges, pageCount: totalPages, allowGaps: gapsOk })
      .then(() => current && setChecked({ key, problem: "" }))
      .catch((e) => current && setChecked({ key, problem: String(e) }));
    return () => {
      current = false;
    };
  }, [key]); // eslint-disable-line react-hooks/exhaustive-deps

  const checking = checked?.key !== key;
  const draftProblem = Object.keys(drafts).length > 0 ? `Enter a page from 1 to ${totalPages}.` : "";
  const problem = draftProblem || (checked && !checking ? checked.problem : "");
  // FR-42: export only after the backend has accepted exactly these bounds.
  const canExport = !checking && !problem && !exporting;

  const cancelSplit = () => {
    setSplitting(null);
    setSplitError("");
  };
  const edit = (next: Range[]) => {
    cancelSplit();
    onChange(next);
  };
  // Adding, joining or deleting parts renumbers them, so unfinished page fields are dropped.
  const restructure = (next: Range[]) => {
    setDrafts({});
    edit(next);
  };

  /** Applies a typed page to part `i`, or keeps the text as a draft until it's a valid page. */
  function typePage(i: number, side: "start" | "end", text: string) {
    const field = `${side}-${i}`;
    const page = parsePage(text, totalPages);
    const { [field]: _, ...others } = drafts;
    if (page === null) return setDrafts({ ...others, [field]: text });
    setDrafts(others);
    edit(side === "start" ? setStart(ranges, i, page) : setEnd(ranges, i, page));
  }

  function confirmSplit() {
    if (!splitting) return;
    const part = ranges[splitting.index];
    const page = parsePage(splitting.page, totalPages);
    const next = page === null ? null : splitAt(ranges, splitting.index, page);
    if (next) {
      restructure(next);
      setFocusNext(focusAfter({ kind: "split", part: splitting.index }));
    } else setSplitError(`Choose a page from ${part.start + 1} to ${part.end} to start the new part.`);
  }

  function deletePart(i: number) {
    setLeftOut([...leftOut, { start: ranges[i].start, end: ranges[i].end }]);
    restructure(removePart(ranges, i));
    setFocusNext(focusAfter({ kind: "delete", part: i, parts: ranges.length }));
  }

  function closeSplit(i: number) {
    cancelSplit();
    setFocusNext(focusAfter({ kind: "close-split", part: i }));
  }

  const skipped = uncoveredPages(ranges, totalPages);

  return (
    <section className="review" aria-label="Review parts" ref={screen}>
      <header className="review-header">
        <h2 tabIndex={-1} data-focus="heading">
          {ranges.length} {ranges.length === 1 ? "part" : "parts"}
        </h2>
        {skipped > 0 && (
          <span className="muted">
            {skipped} {skipped === 1 ? "page" : "pages"} left out
          </span>
        )}
      </header>

      <table className="parts-table">
        <thead>
          <tr>
            <th scope="col">#</th>
            <th scope="col">Title</th>
            <th scope="col">Start</th>
            <th scope="col">End</th>
            <th scope="col">Pages</th>
            <th scope="col">
              <span className="visually-hidden">Actions</span>
            </th>
          </tr>
        </thead>
        <tbody>
          {ranges.map((r, i) => (
            <tr key={i}>
              <td>{i + 1}</td>
              <td>
                <input
                  data-focus={`title-${i}`}
                  aria-label={`Title of part ${i + 1}`}
                  value={r.title}
                  placeholder={`Part ${i + 1}`}
                  onChange={(e) => edit(rename(ranges, i, e.target.value))}
                />
              </td>
              {(["start", "end"] as const).map((side) => {
                const draft = drafts[`${side}-${i}`];
                return (
                  <td key={side}>
                    <input
                      type="number"
                      aria-label={`${side === "start" ? "First" : "Last"} page of part ${i + 1}`}
                      aria-invalid={draft !== undefined}
                      min={1}
                      max={totalPages}
                      value={draft ?? r[side]}
                      onChange={(e) => typePage(i, side, e.target.value)}
                    />
                  </td>
                );
              })}
              <td>{pageCount(r)}</td>
              <td>
                {splitting?.index === i ? (
                  <form
                    className="row"
                    onSubmit={(e) => {
                      e.preventDefault();
                      confirmSplit();
                    }}
                    onKeyDown={(e) => e.key === "Escape" && closeSplit(i)}
                  >
                    <input
                      type="number"
                      autoFocus
                      aria-label={`Page that starts the new part, ${r.start + 1} to ${r.end}`}
                      min={r.start + 1}
                      max={r.end}
                      value={splitting.page}
                      onChange={(e) => setSplitting({ index: i, page: e.target.value })}
                    />
                    <button type="submit">Split</button>
                    <button type="button" onClick={() => closeSplit(i)}>
                      Cancel
                    </button>
                    {splitError && (
                      <p className="error" role="alert">
                        {splitError}
                      </p>
                    )}
                  </form>
                ) : (
                  <div className="row">
                    <button
                      type="button"
                      data-focus={`add-split-${i}`}
                      disabled={pageCount(r) < 2}
                      onClick={() => setSplitting({ index: i, page: String(Math.floor((r.start + r.end + 1) / 2)) })}
                    >
                      Add split
                    </button>
                    <button
                      type="button"
                      disabled={!canJoin(ranges, i)}
                      title={
                        i < ranges.length - 1 && !canJoin(ranges, i)
                          ? "Pages between these parts are left out. Move a boundary to include them first."
                          : undefined
                      }
                      onClick={() => {
                        restructure(mergeWithNext(ranges, i));
                        setFocusNext(focusAfter({ kind: "join", part: i }));
                      }}
                    >
                      Join next
                    </button>
                    <button type="button" disabled={ranges.length === 1} onClick={() => deletePart(i)}>
                      Delete
                    </button>
                  </div>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>

      {problem && (
        <p className="error" role="alert">
          {problem}
        </p>
      )}

      <footer className="row review-actions">
        <button type="button" onClick={onBack}>
          Back
        </button>
        <button type="button" className="primary" disabled={!canExport} onClick={() => canExport && onExport()}>
          {exporting ? "Saving…" : `Export ${ranges.length} ${ranges.length === 1 ? "file" : "files"}`}
        </button>
      </footer>
    </section>
  );
}
