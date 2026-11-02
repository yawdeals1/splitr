// What Smart mode does with the result of run_preset (FR-30 to FR-32, #8).
import type { AiRun, Preset, PresetRun, Range, StrategyKind } from "./types";

/**
 * True if this result of run_preset comes from the page text, so pages without text matter (FR-80):
 * parts found by a pattern or blank pages, or a split that goes on to the AI. Parts from bookmarks
 * or fixed lengths never read the text, whatever the preset falls back to.
 */
export function readPageText(run: PresetRun): boolean {
  return run.kind === "needsAi" || run.strategy === "pattern" || run.strategy === "blank" || run.strategy === "ai";
}

/** The status line while the text is read: the text layer, then OCR of the pages without text. */
export function readingStatus(progress: { done: number; total: number; ocr?: boolean }): string {
  return progress.ocr
    ? `Reading scanned pages (OCR): ${progress.done} of ${progress.total}`
    : `Reading page text: ${progress.done} of ${progress.total}`;
}

/** True if the preset may split at blank pages, so the "drop separators" option applies (FR-32). */
export function usesBlank(preset: Pick<Preset, "strategy" | "fallback"> | null | undefined): boolean {
  return preset?.strategy === "blank" || preset?.fallback === "blank";
}

/** How each rule strategy splits, for the message after a fallback. */
const SPLITS_BY: Record<Exclude<StrategyKind, "ai">, string> = {
  outline: "at its bookmarks",
  pattern: "where the preset's pattern matches",
  blank: "at blank pages",
  fixed: "into parts of equal length",
};

export type SmartResult =
  | { kind: "parts"; ranges: Range[]; allowGaps: boolean; notice: string }
  | { kind: "needsAi"; notice: string };

export function smartResult(run: PresetRun): SmartResult {
  if (run.kind === "needsAi") return { kind: "needsAi", notice: aiFallbackNotice(run.reason) };
  const how = run.strategy === "ai" ? "" : SPLITS_BY[run.strategy];
  const notice = run.fallbackReason && how ? `${run.fallbackReason}, so Splitr split it ${how} instead.` : "";
  return { kind: "parts", ranges: run.ranges, allowGaps: run.allowGaps, notice };
}

/** Shown with the AI's parts when the preset's rules found nothing and it fell back to the AI. */
export function aiFallbackNotice(reason: string | null): string {
  return reason ? `${reason}, so Splitr asked the AI instead.` : "";
}

/** What the review screen opens with after an AI split (FR-36), or null after Cancel. */
export interface AiReview {
  ranges: Range[];
  /** Why the AI's answer couldn't be used; "" when it could. */
  problem: string;
}

export function aiReview(run: AiRun, pageCount: number): AiReview | null {
  switch (run.kind) {
    case "parts":
      return { ranges: run.ranges, problem: "" };
    case "unusable":
      // One part with every page, for the user to split by hand.
      return { ranges: [{ start: 1, end: pageCount, title: "Part 1" }], problem: run.problem };
    case "cancelled":
      return null;
  }
}

/** The status line while the AI step runs: reading the text first, then the requests. */
export function aiStatus(step: { reading: boolean; done: number; total: number; ocr?: boolean }): string {
  if (step.reading) return readingStatus(step);
  if (step.total <= 1) return "Asking the AI…";
  return `Asking the AI: ${step.done} of ${step.total} parts of the file answered`;
}
