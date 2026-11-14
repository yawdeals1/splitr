// Light, dark or system theme and the colour set (FR-63). App.css reads the `data-theme` and
// `data-palette` attributes on <html>.
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Palette, Theme } from "./types";

/**
 * Every colour set, in the order the panel shows them. `swatch` and `swatchDark` are the set's light
 * and dark accents, so each swatch shows the accent the current theme uses.
 */
export const PALETTES: { value: Palette; label: string; swatch: string; swatchDark: string }[] = [
  { value: "classic", label: "Classic", swatch: "#2f5bd3", swatchDark: "#7c9cf0" },
  { value: "steel", label: "Steel", swatch: "#3d5a73", swatchDark: "#8fb3d1" },
  { value: "forest", label: "Forest", swatch: "#2d6a3e", swatchDark: "#7fc490" },
  { value: "ember", label: "Ember", swatch: "#ad4219", swatchDark: "#f2925f" },
  { value: "rose", label: "Rose", swatch: "#bb336a", swatchDark: "#f28ab3" },
  { value: "lavender", label: "Lavender", swatch: "#6b45c2", swatchDark: "#b6a0f5" },
  { value: "plum", label: "Plum", swatch: "#8a3479", swatchDark: "#e29bd4" },
];

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

/** Shows the interface in `palette`. Classic is App.css's base palette, so it needs no attribute. */
export function applyPalette(root: HTMLElement, palette: Palette) {
  if (palette === "classic") delete root.dataset.palette;
  else root.dataset.palette = palette;
}
