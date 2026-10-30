// Unit tests for blank.ts. Run with `npm test`.
import { test } from "node:test";
import assert from "node:assert/strict";
import { MAX_BLANK_THRESHOLD, parseThreshold } from "./blank.ts";

test("parseThreshold accepts whole numbers from 0 to the maximum", () => {
  assert.equal(parseThreshold("0"), 0);
  assert.equal(parseThreshold(" 12 "), 12);
  assert.equal(parseThreshold(String(MAX_BLANK_THRESHOLD)), MAX_BLANK_THRESHOLD);
});

test("parseThreshold refuses anything else", () => {
  for (const bad of ["", "-1", "2.5", "abc", "1e3", String(MAX_BLANK_THRESHOLD + 1)]) {
    assert.equal(parseThreshold(bad), null, bad);
  }
});

test("the maximum matches the backend", async () => {
  const { readFileSync } = await import("node:fs");
  const rust = readFileSync(new URL("../src-tauri/src/settings.rs", import.meta.url), "utf8");
  assert.match(rust, new RegExp(`MAX_BLANK_THRESHOLD: u32 = ${MAX_BLANK_THRESHOLD};`));
});
