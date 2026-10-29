// Unit tests for ai.ts. Run with `npm test` (Node's built-in test runner).
import { test } from "node:test";
import assert from "node:assert/strict";
import { aiBlock, aiBlockMessage, usesAi } from "./ai.ts";

test("usesAi covers AI presets, AI fallbacks and the custom prompt", () => {
  assert.equal(usesAi({ strategy: "ai" }), true);
  assert.equal(usesAi({ strategy: "outline", fallback: "ai" }), true);
  assert.equal(usesAi(null), true);
  assert.equal(usesAi({ strategy: "pattern", fallback: null }), false);
  assert.equal(usesAi({ strategy: "blank" }), false);
});

test("aiBlock checks the switch, then the key, then consent", () => {
  assert.equal(aiBlock({ aiEnabled: false, aiConsent: true }, true), "off");
  assert.equal(aiBlock({ aiEnabled: true, aiConsent: true }, false), "no-key");
  assert.equal(aiBlock({ aiEnabled: true, aiConsent: false }, true), "consent");
  assert.equal(aiBlock({ aiEnabled: true, aiConsent: true }, true), null);
});

test("aiBlockMessage says what to do", () => {
  assert.match(aiBlockMessage("off"), /Use AI/);
  assert.match(aiBlockMessage("no-key"), /API key/);
});
