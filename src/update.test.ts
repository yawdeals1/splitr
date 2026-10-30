// Unit tests for update.ts. Run with `npm test` (Node's built-in test runner).
import { test } from "node:test";
import assert from "node:assert/strict";
import { downloadStep, updateMessage, type UpdateState } from "./update.ts";

test("an available update names its version", () => {
  assert.equal(updateMessage({ kind: "available", version: "0.2.0" }), "Splitr 0.2.0 is available.");
});

test("download progress adds up the chunks and ends in a restart", () => {
  let state: UpdateState = { kind: "available", version: "0.2.0" };
  state = downloadStep(state, { event: "Started", data: { contentLength: 200 } });
  assert.equal(updateMessage(state), "Downloading the update: 0%");
  state = downloadStep(state, { event: "Progress", data: { chunkLength: 50 } });
  state = downloadStep(state, { event: "Progress", data: { chunkLength: 51 } });
  assert.deepEqual(state, { kind: "downloading", received: 101, total: 200 });
  assert.equal(updateMessage(state), "Downloading the update: 50%");
  state = downloadStep(state, { event: "Finished" });
  assert.equal(updateMessage(state), "Installing the update. Splitr will restart.");
});

test("a download without a known size shows no percentage", () => {
  const state = downloadStep({ kind: "available", version: "1" }, { event: "Started", data: {} });
  assert.equal(updateMessage(state), "Downloading the update…");
});

test("a failed update says why", () => {
  assert.equal(
    updateMessage({ kind: "failed", message: "signature mismatch" }),
    "The update could not be installed: signature mismatch",
  );
});
