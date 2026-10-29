// When Smart mode needs the AI, and what stops it from running (FR-60 to FR-62).
import type { Preset, Settings } from "./types";

/** The platform keys are saved for until several platforms are supported (#27). */
export const AI_PLATFORM = "anthropic";
export const AI_PLATFORM_NAME = "Claude";

/**
 * True if splitting with this preset may call the AI: AI presets, presets that fall back to the
 * AI, and the custom prompt (`null`).
 */
export function usesAi(preset: Pick<Preset, "strategy" | "fallback"> | null): boolean {
  return preset === null || preset.strategy === "ai" || preset.fallback === "ai";
}

/** What has to happen before the AI may run, in the order the user meets it. */
export type AiBlock = "off" | "no-key" | "consent" | null;

export function aiBlock(settings: Pick<Settings, "aiEnabled" | "aiConsent">, hasKey: boolean): AiBlock {
  if (!settings.aiEnabled) return "off";
  if (!hasKey) return "no-key";
  if (!settings.aiConsent) return "consent";
  return null;
}

/** The message for a block the user fixes in the AI section of the panel. */
export function aiBlockMessage(block: "off" | "no-key"): string {
  return block === "off"
    ? "AI features are off. Turn on “Use AI” below, or pick a preset that splits by rules."
    : `Add your ${AI_PLATFORM_NAME} API key below to split with this preset.`;
}
