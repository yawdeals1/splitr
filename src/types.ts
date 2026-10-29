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

export type RangeMode = "custom" | "fixed" | "smart";
