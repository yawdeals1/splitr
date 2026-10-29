// Unit tests for thumbnails.ts. Run with `npm test` (Node's built-in test runner).
import { test } from "node:test";
import assert from "node:assert/strict";
import { Cancelled, createQueue, renderWidth } from "./thumbnails.ts";

/** A task that finishes when `finish` is called, recording when it started. */
function deferred(name: string, started: string[]) {
  let finish!: () => void;
  const task = () =>
    new Promise<string>((resolve) => {
      started.push(name);
      finish = () => resolve(name);
    });
  return { task, finish: () => finish() };
}

const tick = () => new Promise((r) => setTimeout(r, 0));

test("createQueue runs at most `limit` tasks, newest first", async () => {
  const schedule = createQueue(2);
  const started: string[] = [];
  const [a, b, c, d] = ["a", "b", "c", "d"].map((n) => deferred(n, started));
  const results = [a, b, c, d].map((t) => schedule(t.task).result);

  assert.deepEqual(started, ["a", "b"]);
  a.finish();
  await tick();
  assert.deepEqual(started, ["a", "b", "d"]);
  b.finish();
  await tick();
  assert.deepEqual(started, ["a", "b", "d", "c"]);
  c.finish();
  d.finish();
  assert.deepEqual(await Promise.all(results), ["a", "b", "c", "d"]);
});

test("cancelling a waiting task skips it and rejects its result", async () => {
  const schedule = createQueue(1);
  const started: string[] = [];
  const a = deferred("a", started);
  const b = deferred("b", started);
  schedule(a.task);
  const skipped = schedule(b.task);

  skipped.cancel();
  await assert.rejects(skipped.result, Cancelled);
  a.finish();
  await tick();
  assert.deepEqual(started, ["a"]);
});

test("cancelling a running task rejects its result and frees its slot", async () => {
  const schedule = createQueue(1);
  const started: string[] = [];
  const a = deferred("a", started);
  const b = deferred("b", started);
  const running = schedule(a.task);
  schedule(b.task);

  running.cancel();
  a.finish();
  await assert.rejects(running.result, Cancelled);
  await tick();
  assert.deepEqual(started, ["a", "b"]);
});

test("a failed task rejects with its error and the queue continues", async () => {
  const schedule = createQueue(1);
  const failed = schedule(() => Promise.reject(new Error("Page 3 could not be shown")));
  const next = schedule(() => Promise.resolve("ok"));
  await assert.rejects(failed.result, /Page 3/);
  assert.equal(await next.result, "ok");
});

test("renderWidth covers the screen pixels in steps of 64", () => {
  assert.equal(renderWidth(150, 1), 192);
  assert.equal(renderWidth(150, 2), 320);
  assert.equal(renderWidth(128, 1), 128);
  assert.equal(renderWidth(0, 1), 64);
  assert.equal(renderWidth(129, 1.25), 192);
});
