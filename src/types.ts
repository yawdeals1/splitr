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

// open_pdf errors that mean "ask for the password" (FR-03). Must match SplitError in error.rs.
export const PASSWORD_REQUIRED = "This PDF is password protected. Enter its password to open it.";
export const WRONG_PASSWORD = "The password is incorrect. Try again.";

export type RangeMode = "custom" | "fixed" | "smart";
