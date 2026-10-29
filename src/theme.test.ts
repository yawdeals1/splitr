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

/**
 * The declarations of the first top-level (or, with `media`, first in that @media block) rule whose
 * whole selector is `selector`, as a property to value map. Whitespace and comments don't matter.
 */
function declarations(selector: string, media?: string): Map<string, string> {
  let source = "} " + css.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\s+/g, " ");
  if (media) {
    const at = source.indexOf(`@media ${media} {`);
    assert.notEqual(at, -1, `no @media ${media} block`);
    source = "} " + source.slice(source.indexOf("{", at) + 1).trimStart();
  }
  const head = [`} ${selector} {`, `}${selector} {`, `} ${selector}{`].map((h) => source.indexOf(h)).find((i) => i >= 0);
  assert.ok(head !== undefined, `no rule for ${selector}`);
  const open = source.indexOf("{", head + 1);
  const body = source.slice(open + 1, source.indexOf("}", open));
  const map = new Map<string, string>();
  for (const part of body.split(";")) {
    const colon = part.indexOf(":");
    if (colon > 0) map.set(part.slice(0, colon).trim(), part.slice(colon + 1).trim());
  }
  return map;
}

/** The custom properties (theme tokens) among a rule's declarations. */
function tokens(map: Map<string, string>): Record<string, string> {
  return Object.fromEntries([...map].filter(([name]) => name.startsWith("--")));
}

test("App.css: every theme defines the same tokens", () => {
  const light = Object.keys(tokens(declarations(":root"))).sort();
  assert.ok(light.length > 0);
  assert.deepEqual(Object.keys(tokens(declarations(':root[data-theme="dark"]'))).sort(), light);
});

test("App.css: the OS dark theme and the forced dark theme use the same palette", () => {
  // The dark palette is written twice (FR-63); this keeps the two copies from drifting apart.
  const os = tokens(declarations(':root:not([data-theme="light"])', "(prefers-color-scheme: dark)"));
  const forced = tokens(declarations(':root[data-theme="dark"]'));
  assert.deepEqual(os, forced);
});

test("App.css: forced themes set color-scheme so native controls match", () => {
  assert.equal(declarations(':root[data-theme="light"]').get("color-scheme"), "light");
  assert.equal(declarations(':root[data-theme="dark"]').get("color-scheme"), "dark");
});

test("App.css: every control shows a visible focus ring", () => {
  assert.equal(declarations(":focus-visible").get("outline"), "2px solid var(--accent)");
});
