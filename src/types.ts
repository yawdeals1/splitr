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

export type Theme = "system" | "light" | "dark";

/** Mirrors settings::Settings (SRS 6.4). API keys are never part of this. */
export interface Settings {
  theme: Theme;
  lastOutputFolder: string | null;
  nameTemplate: string;
  blankThreshold: number;
  aiConsent: boolean;
}
