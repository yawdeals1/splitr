// Light, dark or system theme (FR-63). App.css reads the `data-theme` attribute on <html>.
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Theme } from "./types";

/** The `data-theme` value for a theme; null for "system", so the OS preference applies. */
export function themeAttribute(theme: Theme): "light" | "dark" | null {
  return theme === "system" ? null : theme;
}

/** Shows the interface in `theme`. */
export function applyTheme(root: HTMLElement, theme: Theme) {
  const value = themeAttribute(theme);
  if (value) root.dataset.theme = value;
  else delete root.dataset.theme;
}

/** Shows `theme` in the page and, where the OS allows, in the window's title bar. */
export function showTheme(theme: Theme) {
  applyTheme(document.documentElement, theme);
  getCurrentWindow()
    .setTheme(themeAttribute(theme))
    .catch(() => {});
}
