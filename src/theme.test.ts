// Unit tests for theme.ts and the theme and focus rules in App.css. Run with `npm test`.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { applyTheme, themeAttribute } from "./theme.ts";

const css = readFileSync(new URL("./App.css", import.meta.url), "utf8");

test("themeAttribute leaves the system theme to the OS", () => {
  assert.equal(themeAttribute("system"), null);
  assert.equal(themeAttribute("light"), "light");
  assert.equal(themeAttribute("dark"), "dark");
});

test("applyTheme sets and clears data-theme", () => {
  const root = { dataset: {} as Record<string, string> } as HTMLElement;
  applyTheme(root, "dark");
  assert.equal(root.dataset.theme, "dark");
  applyTheme(root, "light");
  assert.equal(root.dataset.theme, "light");
  applyTheme(root, "system");
  assert.equal("theme" in root.dataset, false);
});

test("App.css: system follows the OS unless light is chosen, and both can be forced", () => {
  assert.match(css, /@media \(prefers-color-scheme: dark\)\s*{\s*:root:not\(\[data-theme="light"\]\)/);
  assert.match(css, /:root\[data-theme="dark"\]\s*{/);
  assert.match(css, /:root\[data-theme="light"\]\s*{\s*color-scheme: light;/);
});

test("App.css: every control shows a visible focus ring", () => {
  assert.match(css, /:focus-visible\s*{\s*outline: 2px solid var\(--accent\);/);
});
