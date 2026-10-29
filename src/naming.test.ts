// Unit tests for naming.ts. Run with `npm test`.
import { test } from "node:test";
import assert from "node:assert/strict";
import { namePreview, savedMessage } from "./naming.ts";

test("namePreview lists the first names and counts the rest", () => {
  assert.deepEqual(namePreview(["a", "b"]), { shown: ["a", "b"], more: 0 });
  assert.deepEqual(namePreview(["a", "b", "c", "d", "e"]), { shown: ["a", "b", "c"], more: 2 });
  assert.deepEqual(namePreview([]), { shown: [], more: 0 });
});

test("savedMessage says where the files went and why names changed", () => {
  assert.equal(savedMessage(1, "D:\\out", 0), "Saved 1 file to D:\\out.");
  assert.equal(
    savedMessage(3, "/out", 1),
    "Saved 3 files to /out. One file got a number added, because the folder already had a file with that name.",
  );
  assert.match(savedMessage(3, "/out", 2), /2 files got a number added/);
  assert.equal(savedMessage(3, "/out", 0, "/out/Book (2).zip"), "Saved 3 files in Book (2).zip to /out.");
});
