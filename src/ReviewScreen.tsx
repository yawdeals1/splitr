// Review and edit the proposed parts before anything is written (FR-40 to FR-42).
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { mergeWithNext, pageCount, removePart, rename, setEnd, setStart, splitAt, uncoveredPages } from "./review";
import type { Range } from "./types";

interface Props {
  totalPages: number;
  ranges: Range[];
  /** True when pages may be left out from the start, as in Custom mode. */
  allowGaps: boolean;
  onChange: (ranges: Range[]) => void;
  onBack: () => void;
  onExport: () => void;
}

export function ReviewScreen({ totalPages, ranges, allowGaps, onChange, onBack, onExport }: Props) {
  const [problem, setProblem] = useState("");
  const [dropped, setDropped] = useState(false);
  const [splitting, setSplitting] = useState<{ index: number; page: string } | null>(null);
  const [splitError, setSplitError] = useState("");
  const gapsOk = allowGaps || dropped;

  // FR-35: every edit is checked by the backend before export is allowed.
  useEffect(() => {
    let current = true;
    invoke("validate_ranges", { ranges, pageCount: totalPages, allowGaps: gapsOk })
      .then(() => current && setProblem(""))
      .catch((e) => current && setProblem(String(e)));
    return () => {
      current = false;
    };
  }, [ranges, totalPages, gapsOk]);

  const edit = (next: Range[]) => {
    setSplitting(null);
    setSplitError("");
    onChange(next);
  };
  const toPage = (value: string) => Math.trunc(Number(value));

  function confirmSplit() {
    if (!splitting) return;
    const next = splitAt(ranges, splitting.index, toPage(splitting.page));
    const part = ranges[splitting.index];
    if (next) edit(next);
    else setSplitError(`Choose a page from ${part.start + 1} to ${part.end} to start the new part.`);
  }

  const skipped = uncoveredPages(ranges, totalPages);

  return (
    <section className="review" aria-label="Review parts">
      <header className="review-header">
        <h2>
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
                  aria-label={`Title of part ${i + 1}`}
                  value={r.title}
                  placeholder={`Part ${i + 1}`}
                  onChange={(e) => edit(rename(ranges, i, e.target.value))}
                />
              </td>
              <td>
                <input
                  type="number"
                  aria-label={`First page of part ${i + 1}`}
                  min={1}
                  max={totalPages}
                  value={r.start}
                  onChange={(e) => edit(setStart(ranges, i, toPage(e.target.value)))}
                />
              </td>
              <td>
                <input
                  type="number"
                  aria-label={`Last page of part ${i + 1}`}
                  min={1}
                  max={totalPages}
                  value={r.end}
                  onChange={(e) => edit(setEnd(ranges, i, toPage(e.target.value)))}
                />
              </td>
              <td>{pageCount(r)}</td>
              <td>
                {splitting?.index === i ? (
                  <form
                    className="row"
                    onSubmit={(e) => {
                      e.preventDefault();
                      confirmSplit();
                    }}
                    onKeyDown={(e) => e.key === "Escape" && edit(ranges)}
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
                    <button type="button" onClick={() => edit(ranges)}>
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
                      disabled={pageCount(r) < 2}
                      onClick={() => setSplitting({ index: i, page: String(Math.floor((r.start + r.end + 1) / 2)) })}
                    >
                      Add split
                    </button>
                    <button type="button" disabled={i === ranges.length - 1} onClick={() => edit(mergeWithNext(ranges, i))}>
                      Join next
                    </button>
                    <button
                      type="button"
                      disabled={ranges.length === 1}
                      onClick={() => {
                        setDropped(true);
                        edit(removePart(ranges, i));
                      }}
                    >
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
        <button type="button" className="primary" disabled={!!problem} onClick={onExport}>
          Export {ranges.length} {ranges.length === 1 ? "file" : "files"}
        </button>
      </footer>
    </section>
  );
}
