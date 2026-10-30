// Unit tests for smart.ts. Run with `npm test` (Node's built-in test runner).
import { test } from "node:test";
import assert from "node:assert/strict";
import { aiFallbackNotice, aiReview, aiStatus, smartResult, usesBlank } from "./smart.ts";

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

test("an AI preset needs the AI, and a fallback to it says why", () => {
  assert.deepEqual(smartResult({ kind: "needsAi", reason: null }), { kind: "needsAi", notice: "" });
  assert.deepEqual(smartResult({ kind: "needsAi", reason: "This PDF has no bookmarks" }), {
    kind: "needsAi",
    notice: "This PDF has no bookmarks, so Splitr asked the AI instead.",
  });
  assert.equal(aiFallbackNotice(null), "");
});

test("the AI's parts open the review screen as they are", () => {
  const ranges = [{ start: 1, end: 3, title: "A" }];
  assert.deepEqual(aiReview({ kind: "parts", ranges }, 3), { ranges, problem: "" });
});

test("an unusable answer opens the review screen with one part and the problem (FR-36)", () => {
  const problem = "The AI's answer for pages 1 to 9 could not be used: it has no parts. Set the parts by hand instead";
  assert.deepEqual(aiReview({ kind: "unusable", problem }, 9), {
    ranges: [{ start: 1, end: 9, title: "Part 1" }],
    problem,
  });
});

test("cancel returns to the options", () => {
  assert.equal(aiReview({ kind: "cancelled" }, 9), null);
});

test("the status says what the AI step is doing", () => {
  assert.equal(aiStatus({ reading: true, done: 3, total: 40 }), "Reading page text: 3 of 40");
  assert.equal(aiStatus({ reading: false, done: 0, total: 1 }), "Asking the AI…");
  assert.equal(aiStatus({ reading: false, done: 1, total: 3 }), "Asking the AI: 1 of 3 parts of the file answered");
});
