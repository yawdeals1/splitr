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

/** Mirrors keystore::KeyStatus: all the interface learns about a saved API key (NFR-05). */
export interface KeyStatus {
  platform: string;
  /** The key's last 4 characters, or null when none is saved. */
  last4: string | null;
}
