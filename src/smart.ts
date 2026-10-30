// What Smart mode does with the result of run_preset (FR-30 to FR-32, #8).
import type { Preset, PresetRun, Range, StrategyKind } from "./types";

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
  | { kind: "needsAi"; message: string };

export function smartResult(run: PresetRun): SmartResult {
  if (run.kind === "needsAi") return { kind: "needsAi", message: aiNotYet(run.reason) };
  const how = run.strategy === "ai" ? "" : SPLITS_BY[run.strategy];
  const notice = run.fallbackReason && how ? `${run.fallbackReason}, so Splitr split it ${how} instead.` : "";
  return { kind: "parts", ranges: run.ranges, allowGaps: run.allowGaps, notice };
}

/** Shown once the AI may run but Smart mode can't call it yet (#64). */
export function aiNotYet(reason: string | null): string {
  const why = reason ? `${reason}, so this preset needs the AI. ` : "";
  return `${why}Splitting with AI isn't available in this version yet. Try Custom or Fixed mode.`;
}
