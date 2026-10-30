// Unit tests for ai.ts. Run with `npm test` (Node's built-in test runner).
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { aiBlock, aiBlockMessage, aiService, keyChoices, usesAi, usesKey } from "./ai.ts";

test("usesAi covers AI presets, AI fallbacks and the custom prompt", () => {
  assert.equal(usesAi({ strategy: "ai" }), true);
  assert.equal(usesAi({ strategy: "outline", fallback: "ai" }), true);
  assert.equal(usesAi(null), true);
  assert.equal(usesAi({ strategy: "pattern", fallback: null }), false);
  assert.equal(usesAi({ strategy: "blank" }), false);
});

test("aiBlock checks the switch, then the account or key, then consent", () => {
  const claude = { aiPlatform: "anthropic" };
  assert.equal(aiBlock({ ...claude, aiEnabled: false, aiConsent: true }, true), "off");
  assert.equal(aiBlock({ ...claude, aiEnabled: true, aiConsent: true }, false), "no-key");
  assert.equal(aiBlock({ ...claude, aiEnabled: true, aiConsent: false }, true), "consent");
  assert.equal(aiBlock({ ...claude, aiEnabled: true, aiConsent: true }, true), null);
  assert.equal(aiBlock({ aiPlatform: "chatgpt", aiEnabled: true, aiConsent: true }, true), "no-account");
});

test("aiBlockMessage says what to do", () => {
  const openai = aiService("openai");
  assert.match(aiBlockMessage("off", openai), /Use AI/);
  assert.equal(aiBlockMessage("no-key", openai), "Add your OpenAI API key below to split with this preset.");
  assert.match(aiBlockMessage("no-account", openai), /ChatGPT/);
});

test("aiService finds a service, and falls back to the ChatGPT account", () => {
  assert.equal(aiService("anthropic").company, "Anthropic");
  assert.equal(aiService("openai").name, "OpenAI");
  assert.equal(aiService("gemini").company, "Google");
  assert.equal(aiService("someday").id, "chatgpt");
  assert.equal(aiService(undefined).id, "chatgpt");
  assert.equal(usesKey(aiService("openai")), true);
  assert.equal(usesKey(aiService("chatgpt")), false);
  assert.equal(usesKey(aiService("mistral")), true);
});

test("the OpenAI-compatible services match the backend's data file", () => {
  const file = new URL("../src-tauri/src/ai/compatible.json", import.meta.url);
  const platforms: { id: string; name: string; company: string; keyHint: string }[] = JSON.parse(readFileSync(file, "utf8"));
  assert.ok(platforms.length > 0);
  for (const p of platforms) {
    const service = aiService(p.id);
    assert.equal(service.id, p.id, `${p.id} is missing from AI_SERVICES`);
    assert.deepEqual([service.name, service.company, service.keyHint], [p.name, p.company, p.keyHint]);
    assert.equal(service.listsModels, true);
  }
  assert.equal(aiService("custom").listsModels, true);
  assert.equal(aiService("opencode-go").listsModels, true);
});

test("keyChoices preselects a detected platform and lets the user change it", () => {
  const found = keyChoices({ kind: "platform", platform: "openai" });
  assert.equal(found.detected, true);
  assert.equal(found.platform, "openai");
  assert.equal(found.options[0], "openai");
  assert.ok(found.options.includes("gemini") && !found.options.includes("chatgpt"));
});

test("keyChoices makes the user pick when a prefix fits several platforms", () => {
  const unsure = keyChoices({ kind: "ambiguous", options: ["openai", "deepseek"] });
  assert.equal(unsure.platform, "");
  assert.deepEqual(unsure.options.slice(0, 2), ["openai", "deepseek"]);
  assert.equal(new Set(unsure.options).size, unsure.options.length);
});

test("keyChoices explains a key of the wrong kind", () => {
  const wrong = keyChoices({ kind: "rejected", reason: "This is an OpenAI Admin key" });
  assert.deepEqual(wrong.options, []);
  assert.match(wrong.problem, /Admin key/);
});
