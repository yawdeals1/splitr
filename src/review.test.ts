// Unit tests for review.ts. Run with `npm test` (Node's built-in test runner).
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  canJoin,
  gaps,
  mergeWithNext,
  pageCount,
  parsePage,
  removePart,
  rename,
  setEnd,
  setStart,
  splitAt,
  uncoveredPages,
  within,
} from "./review.ts";
import type { Range } from "./types.ts";

const r = (start: number, end: number, title = ""): Range => ({ start, end, title });
const parts = [r(1, 4, "A"), r(5, 9, "B"), r(10, 12, "C")];

test("moves a shared boundary from either side", () => {
  assert.deepEqual(setEnd(parts, 0, 6), [r(1, 6, "A"), r(7, 9, "B"), r(10, 12, "C")]);
  assert.deepEqual(setStart(parts, 2, 8), [r(1, 4, "A"), r(5, 7, "B"), r(8, 12, "C")]);
});

test("leaves a neighbour alone when there is a gap", () => {
  const gapped = [r(1, 3), r(6, 9)];
  assert.deepEqual(setEnd(gapped, 0, 4), [r(1, 4), r(6, 9)]);
  assert.deepEqual(setStart(gapped, 1, 5), [r(1, 3), r(5, 9)]);
});

test("does not mutate its input", () => {
  const copy = structuredClone(parts);
  setEnd(parts, 0, 6);
  splitAt(parts, 1, 7);
  mergeWithNext(parts, 0);
  removePart(parts, 1);
  rename(parts, 0, "Z");
  assert.deepEqual(parts, copy);
});

test("adds a split inside a part", () => {
  assert.deepEqual(splitAt(parts, 1, 7), [r(1, 4, "A"), r(5, 6, "B"), r(7, 9), r(10, 12, "C")]);
  assert.deepEqual(splitAt(parts, 1, 9), [r(1, 4, "A"), r(5, 8, "B"), r(9, 9), r(10, 12, "C")]);
});

test("refuses a split outside the part or on its first page", () => {
  assert.equal(splitAt(parts, 1, 5), null);
  assert.equal(splitAt(parts, 1, 10), null);
  assert.equal(splitAt(parts, 1, 6.5), null);
});

test("removes a split by merging with the next part", () => {
  assert.deepEqual(mergeWithNext(parts, 0), [r(1, 9, "A"), r(10, 12, "C")]);
  assert.equal(mergeWithNext(parts, 2), parts);
});

test("never joins across pages that are left out", () => {
  const gapped = [r(1, 3), r(6, 9)];
  assert.equal(canJoin(gapped, 0), false);
  assert.equal(mergeWithNext(gapped, 0), gapped);
  assert.equal(canJoin(parts, 0), true);
  assert.equal(canJoin(parts, 2), false);
});

test("deletes and renames parts", () => {
  assert.deepEqual(removePart(parts, 1), [r(1, 4, "A"), r(10, 12, "C")]);
  assert.deepEqual(rename(parts, 2, "Appendix")[2], r(10, 12, "Appendix"));
});

test("counts pages", () => {
  assert.equal(pageCount(r(5, 9)), 5);
  assert.equal(pageCount(r(9, 5)), 0);
  assert.equal(uncoveredPages(parts, 12), 0);
  assert.equal(uncoveredPages(removePart(parts, 1), 12), 5);
  assert.equal(uncoveredPages([r(3, 5), r(4, 8), r(0, 1), r(20, 30)], 10), 3);
  assert.equal(uncoveredPages([r(1, 2_000_000)], 2_000_000), 0);
});

test("finds the runs of pages no part covers", () => {
  assert.deepEqual(gaps([r(3, 5), r(9, 9)], 12), [
    { start: 1, end: 2 },
    { start: 6, end: 8 },
    { start: 10, end: 12 },
  ]);
  assert.deepEqual(gaps(parts, 12), []);
});

test("checks that gaps are only pages the user deleted", () => {
  const deleted = [{ start: 5, end: 9 }];
  assert.equal(within(gaps(removePart(parts, 1), 12), deleted, 12), true);
  // After the delete, moving a boundary opens a new gap the user didn't ask for.
  const moved = setEnd(removePart(parts, 1), 0, 2);
  assert.equal(within(gaps(moved, 12), deleted, 12), false);
  assert.equal(within([], deleted, 12), true);
});

test("accepts only whole pages within the document", () => {
  assert.equal(parsePage("7", 12), 7);
  assert.equal(parsePage(" 12 ", 12), 12);
  for (const text of ["", "0", "-2", "13", "1e10", "2.5", "abc"]) assert.equal(parsePage(text, 12), null, text);
});
