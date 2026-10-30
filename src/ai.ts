// When Smart mode needs the AI, and what stops it from running (FR-60 to FR-62).
import type { KeyDetection, Preset, Settings } from "./types";

/** A service Smart mode can use. Key platforms match the backend's `keystore` and `ai` ids. */
export interface AiService {
  id: string;
  /** What the service list shows. */
  label: string;
  /** The platform's name, such as "OpenAI". */
  name: string;
  /** The company that receives the key and page excerpts (FR-61). */
  company: string;
  /** How a key starts, as a hint in the key field; "" if keys have no known start. */
  keyHint: string;
  /** True for the ChatGPT account, which signs in instead of using a key. */
  signIn?: boolean;
  /** True for OpenAI-compatible platforms, whose model list Splitr can fetch (#30). */
  listsModels?: boolean;
}

/** The services Smart mode can be set to. ChatGPT is the default; #27 adds key management. */
export const AI_SERVICES: AiService[] = [
  { id: "chatgpt", label: "ChatGPT account", name: "ChatGPT", company: "OpenAI", keyHint: "", signIn: true },
  { id: "anthropic", label: "Claude API key", name: "Claude", company: "Anthropic", keyHint: "sk-ant-…" },
  { id: "openai", label: "OpenAI API key", name: "OpenAI", company: "OpenAI", keyHint: "sk-proj-…" },
  { id: "gemini", label: "Gemini API key", name: "Gemini", company: "Google", keyHint: "AIza…" },
  // OpenAI-compatible platforms (#30). Names, companies and hints match src-tauri/src/ai/compatible.json.
  ...[
    { id: "openrouter", name: "OpenRouter", company: "OpenRouter", keyHint: "sk-or-v1-…" },
    { id: "nvidia", name: "NVIDIA", company: "NVIDIA", keyHint: "nvapi-…" },
    { id: "groq", name: "Groq", company: "Groq", keyHint: "gsk_…" },
    { id: "xai", name: "xAI", company: "xAI", keyHint: "xai-…" },
    { id: "deepseek", name: "DeepSeek", company: "DeepSeek", keyHint: "sk-…" },
    { id: "mistral", name: "Mistral", company: "Mistral AI", keyHint: "" },
    { id: "together", name: "Together AI", company: "Together AI", keyHint: "" },
  ].map((p) => ({ ...p, label: `${p.name} API key`, listsModels: true })),
  {
    id: "opencode-go",
    label: "OpenCode Go subscription",
    name: "OpenCode Go",
    company: "OpenCode",
    keyHint: "",
    listsModels: true,
  },
  {
    id: "custom",
    label: "Other OpenAI-compatible platform",
    name: "Custom platform",
    company: "the platform at the address you entered",
    keyHint: "",
    listsModels: true,
  },
];

/** The service with this id, or the ChatGPT account for an id this version doesn't know. */
export function aiService(id: string | undefined): AiService {
  return AI_SERVICES.find((s) => s.id === id) ?? AI_SERVICES[0];
}

/** True for services that use an API key rather than a sign-in. */
export function usesKey(service: AiService): boolean {
  return !service.signIn;
}

/** What to offer after a pasted key's prefix was checked (#27). */
export interface KeyChoices {
  /** True when the prefix named one platform. */
  detected: boolean;
  /** The platform to preselect; "" when the user must pick, because the prefix fits several. */
  platform: string;
  /** Platforms to choose from, the likely ones first. Empty for a key of the wrong kind. */
  options: string[];
  /** Why the key can't be saved, for a key of the wrong kind. */
  problem: string;
}

export function keyChoices(detection: KeyDetection): KeyChoices {
  const all = AI_SERVICES.filter(usesKey).map((s) => s.id);
  const first = (likely: string[]) => [...likely, ...all.filter((id) => !likely.includes(id))];
  switch (detection.kind) {
    case "rejected":
      return { detected: false, platform: "", options: [], problem: detection.reason };
    case "platform":
      return { detected: true, platform: detection.platform, options: first([detection.platform]), problem: "" };
    case "ambiguous":
      // Never guessed: a key goes only to the platform the user picks (NFR-05).
      return { detected: false, platform: "", options: first(detection.options), problem: "" };
  }
}

/**
 * True if splitting with this preset may call the AI: AI presets, presets that fall back to the
 * AI, and the custom prompt (`null`).
 */
export function usesAi(preset: Pick<Preset, "strategy" | "fallback"> | null): boolean {
  return preset === null || preset.strategy === "ai" || preset.fallback === "ai";
}

/** What has to happen before the AI may run, in the order the user meets it. */
export type AiBlock = "off" | "no-account" | "no-key" | "consent" | null;

/** `ready` is true when the chosen service has a saved key, or for ChatGPT, a connected account. */
export function aiBlock(
  settings: Pick<Settings, "aiEnabled" | "aiConsent" | "aiPlatform">,
  ready: boolean,
): AiBlock {
  if (!settings.aiEnabled) return "off";
  if (!ready) return aiService(settings.aiPlatform).signIn ? "no-account" : "no-key";
  if (!settings.aiConsent) return "consent";
  return null;
}

/** The message for a block the user fixes in the AI section of the panel. */
export function aiBlockMessage(block: "off" | "no-account" | "no-key", service: AiService): string {
  if (block === "no-account") return "Connect your ChatGPT account under AI below to split with this preset.";
  return block === "off"
    ? "AI features are off. Turn on “Use AI” below, or pick a preset that splits by rules."
    : `Add your ${service.name} API key below to split with this preset.`;
}
