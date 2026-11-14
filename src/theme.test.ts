// Unit tests for theme.ts and the theme and focus rules in App.css. Run with `npm test`.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { applyPalette, applyTheme, PALETTES, themeAttribute } from "./theme.ts";

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

test("applyPalette sets data-palette, and Classic clears it", () => {
  const root = { dataset: {} as Record<string, string> } as HTMLElement;
  applyPalette(root, "rose");
  assert.equal(root.dataset.palette, "rose");
  applyPalette(root, "classic");
  assert.equal("palette" in root.dataset, false);
});

test("PALETTES lists every colour set in settings::Palette, Classic first", () => {
  const rust = readFileSync(new URL("../src-tauri/src/settings.rs", import.meta.url), "utf8");
  const body = rust.slice(rust.indexOf("pub enum Palette {"), rust.indexOf("}", rust.indexOf("pub enum Palette {")));
  const variants = [...body.matchAll(/^\s+([A-Z]\w*),/gm)].map((m) => m[1].toLowerCase());
  assert.deepEqual(
    PALETTES.map((p) => p.value),
    variants,
  );
  assert.equal(PALETTES[0].value, "classic");
});

/**
 * The declarations of the first top-level (or, with `media`, first in that @media block) rule whose
 * whole selector is `selector`, as a property to value map. Whitespace and comments don't matter.
 */
function declarations(selector: string, media?: string): Map<string, string> {
  let source = "} " + css.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\s+/g, " ");
  if (media) {
    // The first such @media block whose first rule is `selector`, else the first such block.
    const blocks = source.split(`@media ${media} {`).slice(1).map((rest) => "} " + rest.trimStart());
    assert.notEqual(blocks.length, 0, `no @media ${media} block`);
    source = blocks.find((b) => b.startsWith(`} ${selector} {`) || b.startsWith(`} ${selector}{`)) ?? blocks[0];
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

/** WCAG contrast ratio of two #rrggbb colours. */
function contrast(a: string, b: string): number {
  const luminance = (hex: string) => {
    const [r, g, b] = [1, 3, 5].map((i) => {
      const c = parseInt(hex.slice(i, i + 2), 16) / 255;
      return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
    });
    return 0.2126 * r + 0.7152 * g + 0.0722 * b;
  };
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

/** A palette's tokens with the Classic tokens it doesn't set filled in. */
function complete(own: Record<string, string>, base: Record<string, string>) {
  return { ...base, ...own };
}

test("App.css: every colour set has a light, an OS dark and a forced dark palette that match", () => {
  for (const { value } of PALETTES.slice(1)) {
    const light = tokens(declarations(`:root[data-palette="${value}"]`));
    const os = tokens(declarations(`:root[data-palette="${value}"]:not([data-theme="light"])`, "(prefers-color-scheme: dark)"));
    const forced = tokens(declarations(`:root[data-palette="${value}"][data-theme="dark"]`));
    assert.ok(Object.keys(light).length > 0, value);
    assert.deepEqual(Object.keys(forced).sort(), Object.keys(light).sort(), value);
    assert.deepEqual(os, forced, value);
    const swatch = PALETTES.find((p) => p.value === value)?.swatch;
    assert.equal(light["--accent"], swatch, `${value}: the panel swatch is the light accent`);
  }
});

test("App.css: every colour set keeps WCAG AA contrast in both themes", () => {
  const classicLight = tokens(declarations(":root"));
  const classicDark = tokens(declarations(':root[data-theme="dark"]'));
  const sets: [string, Record<string, string>][] = [
    ["classic light", classicLight],
    ["classic dark", classicDark],
  ];
  for (const { value } of PALETTES.slice(1)) {
    sets.push([`${value} light`, complete(tokens(declarations(`:root[data-palette="${value}"]`)), classicLight)]);
    sets.push([`${value} dark`, complete(tokens(declarations(`:root[data-palette="${value}"][data-theme="dark"]`)), classicDark)]);
  }
  for (const [name, t] of sets) {
    for (const [fg, bg] of [
      ["--text", "--bg"],
      ["--text", "--surface"],
      ["--muted", "--bg"],
      ["--accent-text", "--accent"],
    ]) {
      const ratio = contrast(t[fg], t[bg]);
      assert.ok(ratio >= 4.5, `${name}: ${fg} on ${bg} is ${ratio.toFixed(2)}:1`);
    }
  }
});
