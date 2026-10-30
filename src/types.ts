// Mirrors the Rust types in src-tauri/src (SRS 6).

export type StrategyKind = "outline" | "pattern" | "blank" | "fixed" | "ai";

export interface Preset {
  id: string;
  category: string;
  name: string;
  strategy: StrategyKind;
  fallback?: StrategyKind | null;
  prompt: string;
  pattern?: string | null;
  /** Pages in each part, for the fixed strategy (FR-21). */
  every?: number | null;
  /** True for presets the user saved (FR-71). */
  user?: boolean;
}

/** Mirrors user_presets::PresetList: every preset, and why the user's own are missing if they are. */
export interface PresetList {
  presets: Preset[];
  problem: string | null;
}

/** What the interface sends to create (no id) or edit a user preset. */
export interface PresetInput {
  id: string | null;
  category: string;
  name: string;
  prompt: string;
}

export interface Range {
  start: number;
  end: number;
  title: string;
}

export interface PdfInfo {
  path: string;
  fileName: string;
  pageCount: number;
  sizeBytes: number;
}

/** Mirrors strategy::OutlineEntry: a top-level bookmark and the page it opens. */
export interface OutlineEntry {
  title: string;
  page: number;
}

/** Mirrors commands::TextSummary, returned by extract_text (#7). */
export interface TextSummary {
  pageCount: number;
  /** Pages with any visible text. Scanned pages have none. */
  pagesWithText: number;
  outline: OutlineEntry[];
}

/** Mirrors preset_run::RunOptions: what the user set for a Smart split (FR-32). */
export interface RunOptions {
  blankThreshold: number;
  dropSeparators: boolean;
}

/** Mirrors preset_run::PresetRun, returned by run_preset (#8). */
export type PresetRun =
  | {
      kind: "parts";
      ranges: Range[];
      /** The strategy that found the parts: the preset's own, or its fallback. */
      strategy: StrategyKind;
      /** True when dropped blank pages leave gaps. */
      allowGaps: boolean;
      /** Why the preset's own strategy found nothing, when its fallback found the parts. */
      fallbackReason: string | null;
    }
  /** The preset splits with the AI, or falls back to it; `reason` says why the rules found nothing. */
  | { kind: "needsAi"; reason: string | null };

/** Mirrors strategy::AiRun, returned by run_ai (#64). */
export type AiRun =
  | { kind: "parts"; ranges: Range[] }
  /** The AI's answer still failed after the retry: the user sets the parts by hand (FR-36). */
  | { kind: "unusable"; problem: string }
  /** The user pressed Cancel. */
  | { kind: "cancelled" };

/** The payload of the "extract-progress" event that extract_text sends, and of "ai-progress" (chunks answered). */
export interface ExtractProgress {
  done: number;
  total: number;
}

// open_pdf errors that mean "ask for the password" (FR-03). Must match SplitError in error.rs.
export const PASSWORD_REQUIRED = "This PDF is password protected. Enter its password to open it.";
export const WRONG_PASSWORD = "The password is incorrect. Try again.";

export type RangeMode = "custom" | "fixed" | "smart";

export type Theme = "system" | "light" | "dark";

/** Mirrors settings::Settings (SRS 6.4). API keys are never part of this. */
export interface Settings {
  theme: Theme;
  lastOutputFolder: string | null;
  nameTemplate: string;
  blankThreshold: number;
  /** AI features are on (FR-62). */
  aiEnabled: boolean;
  aiConsent: boolean;
  /** The AI service Smart mode uses: "chatgpt" (the default) or a key platform such as "anthropic". */
  aiPlatform: string;
  /** Models the user chose, by service. A service that isn't listed uses its default. */
  aiModels: Record<string, string>;
  /** The address of the custom OpenAI-compatible platform (#30). */
  aiCustomUrl: string;
}

/** Mirrors chatgpt::Status: all the interface learns about a connected ChatGPT account (#31, NFR-05). */
export interface ChatGptStatus {
  connected: boolean;
  email: string | null;
  name: string | null;
  /** The ChatGPT plan, such as "plus", when OpenAI names it. */
  plan: string | null;
  /** False when the user signed in but didn't allow Splitr to use their plan. */
  planUsage: boolean;
}

/** Mirrors keystore::KeyStatus: all the interface learns about a saved API key (NFR-05). */
export interface KeyStatus {
  platform: string;
  /** The key's last 4 characters, or null when none is saved. */
  last4: string | null;
}

/** Mirrors export::ExportRequest: which parts to save, where, and their names without ".pdf". */
export interface ExportRequest {
  path: string;
  password?: string;
  ranges: Range[];
  folder: string;
  names: string[];
}

/** Mirrors naming::ExportNames: the names for one export, without extensions (FR-51 to FR-53). */
export interface ExportNames {
  parts: string[];
  zip: string;
}

/** Mirrors export::SavedParts: the files an export wrote, and how many got a number added (FR-52). */
export interface SavedParts {
  paths: string[];
  renamed: number;
}

/** Mirrors key_detect::Platform. */
export type Platform =
  | "anthropic"
  | "openai"
  | "gemini"
  | "openrouter"
  | "nvidia"
  | "groq"
  | "xai"
  | "deepseek"
  | "mistral"
  | "together"
  | "opencode-go"
  | "custom";

/** Mirrors ai::verify::KeyCheck: what checking a saved key found (#27). */
export type KeyCheck = { status: "connected" } | { status: "unchecked"; reason: string };

/** Mirrors key_detect::Detection: what a key's prefix says about it. */
export type KeyDetection =
  | { kind: "platform"; platform: Platform }
  | { kind: "ambiguous"; options: Platform[] }
  | { kind: "rejected"; reason: string };
