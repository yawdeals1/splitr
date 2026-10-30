// Unit tests for smart.ts. Run with `npm test` (Node's built-in test runner).
import { test } from "node:test";
import assert from "node:assert/strict";
import { aiNotYet, smartResult, usesBlank } from "./smart.ts";

test("the drop separators option applies to blank presets and blank fallbacks", () => {
  assert.equal(usesBlank({ strategy: "blank", fallback: "ai" }), true);
  assert.equal(usesBlank({ strategy: "outline", fallback: "blank" }), true);
  assert.equal(usesBlank({ strategy: "pattern", fallback: "ai" }), false);
  assert.equal(usesBlank(null), false);
});

test("parts from the preset's own strategy come with no notice", () => {
  const ranges = [{ start: 1, end: 4, title: "One" }];
  const run = { kind: "parts", ranges, strategy: "outline", allowGaps: false, fallbackReason: null } as const;
  assert.deepEqual(smartResult(run), { kind: "parts", ranges, allowGaps: false, notice: "" });
});

test("parts from a fallback say why and how", () => {
  const run = {
    kind: "parts",
    ranges: [],
    strategy: "blank",
    allowGaps: true,
    fallbackReason: "This PDF has no bookmarks",
  } as const;
  const result = smartResult(run);
  assert.equal(result.kind, "parts");
  assert.equal(result.kind === "parts" && result.notice, "This PDF has no bookmarks, so Splitr split it at blank pages instead.");
  assert.equal(result.kind === "parts" && result.allowGaps, true);
});

test("an AI preset or AI fallback says the AI isn't available yet", () => {
  assert.deepEqual(smartResult({ kind: "needsAi", reason: null }), { kind: "needsAi", message: aiNotYet(null) });
  assert.match(aiNotYet(null), /^Splitting with AI isn't available/);
  assert.match(aiNotYet("No split points were found"), /^No split points were found, so this preset needs the AI\. /);
});
