// Unit tests for gridKeys.ts. Run with `npm test`.
import { test } from "node:test";
import assert from "node:assert/strict";
import { columnCount, gridMove } from "./gridKeys.ts";

test("gridMove moves by one page left and right, and by a row up and down", () => {
  assert.equal(gridMove("ArrowRight", 5, 20, 4), 6);
  assert.equal(gridMove("ArrowLeft", 5, 20, 4), 4);
  assert.equal(gridMove("ArrowDown", 5, 20, 4), 9);
  assert.equal(gridMove("ArrowUp", 5, 20, 4), 1);
});

test("gridMove stops at the first and last page", () => {
  assert.equal(gridMove("ArrowLeft", 1, 20, 4), 1);
  assert.equal(gridMove("ArrowUp", 3, 20, 4), 3);
  assert.equal(gridMove("ArrowRight", 20, 20, 4), 20);
  assert.equal(gridMove("ArrowDown", 18, 20, 4), 18);
});

test("gridMove goes down into a short last row at its last page", () => {
  // 10 pages in 4 columns: the last row holds pages 9 and 10.
  assert.equal(gridMove("ArrowDown", 7, 10, 4), 10);
  assert.equal(gridMove("ArrowDown", 6, 10, 4), 10);
  assert.equal(gridMove("ArrowDown", 10, 10, 4), 10);
});

test("gridMove jumps to the ends with Home and End", () => {
  assert.equal(gridMove("Home", 9, 20, 4), 1);
  assert.equal(gridMove("End", 9, 20, 4), 20);
});

test("gridMove ignores other keys", () => {
  assert.equal(gridMove("Enter", 9, 20, 4), null);
  assert.equal(gridMove("a", 9, 20, 4), null);
  assert.equal(gridMove("toString", 9, 20, 4), null);
  assert.equal(gridMove("constructor", 9, 20, 4), null);
});

test("columnCount reads the computed grid columns", () => {
  assert.equal(columnCount("140px 140px 140px"), 3);
  assert.equal(columnCount("152.5px"), 1);
  assert.equal(columnCount(""), 1);
});
