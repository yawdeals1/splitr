// Light, dark or system theme and the colour set (FR-63). App.css reads the `data-theme` and
// `data-palette` attributes on <html>.
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Palette, Theme } from "./types";

/** Every colour set, in the order the panel shows them. `swatch` is the set's light accent. */
export const PALETTES: { value: Palette; label: string; swatch: string }[] = [
  { value: "classic", label: "Classic", swatch: "#2f5bd3" },
  { value: "steel", label: "Steel", swatch: "#3d5a73" },
  { value: "forest", label: "Forest", swatch: "#2d6a3e" },
  { value: "ember", label: "Ember", swatch: "#ad4219" },
  { value: "rose", label: "Rose", swatch: "#bb336a" },
  { value: "lavender", label: "Lavender", swatch: "#6b45c2" },
  { value: "plum", label: "Plum", swatch: "#8a3479" },
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
