// Blank page sensitivity (FR-32): how many visible characters, such as a page number, a page may
// have and still count as a blank separator for "Split at blank pages". Saved in settings (SRS 6.4).
import { useEffect, useState } from "react";
import { MAX_BLANK_THRESHOLD, parseThreshold } from "./blank";

interface Props {
  /** The saved threshold, or undefined while settings load. */
  saved: number | undefined;
  /** Saves a new threshold and returns the problem, if any. */
  onSave: (threshold: number) => Promise<string | null>;
}

export function BlankSensitivity({ saved, onSave }: Props) {
  const [draft, setDraft] = useState(String(saved ?? 0));
  const [problem, setProblem] = useState("");
  useEffect(() => setDraft(String(saved ?? 0)), [saved]);

  async function commit() {
    const value = parseThreshold(draft);
    if (value === null) return setProblem(`Enter a whole number from 0 to ${MAX_BLANK_THRESHOLD}.`);
    setProblem("");
    if (value !== saved) setProblem((await onSave(value)) ?? "");
  }

  return (
    <label className="blank-sensitivity">
      Blank page sensitivity
      <input
        type="number"
        min={0}
        max={MAX_BLANK_THRESHOLD}
        value={draft}
        disabled={saved === undefined}
        aria-invalid={problem !== ""}
        aria-describedby="blank-help"
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => e.key === "Enter" && commit()}
      />
      <span id="blank-help" className="muted">
        A page with up to this many characters, not counting spaces, still counts as blank. Raise it if your
        separator pages carry a page number or a short note.
      </span>
      {problem && (
        <span className="error" role="alert">
          {problem}
        </span>
      )}
    </label>
  );
}
