// Unit tests for marks.ts. Run with `npm test` (Node's built-in test runner).
import { test } from "node:test";
import assert from "node:assert/strict";
import { pageMarks, tileMark } from "./marks.ts";

const range = (start: number, end: number, title = "") => ({ start, end, title });

test("pageMarks marks nothing before there are parts", () => {
  assert.deepEqual(pageMarks([], 5), []);
});

test("pageMarks marks where each part starts", () => {
  const marks = pageMarks([range(1, 2, "Intro"), range(3, 4, "  Results ")], 4);
  assert.deepEqual(marks, [
    { part: 1, starts: true, title: "Intro" },
    { part: 1, starts: false, title: "" },
    { part: 2, starts: true, title: "Results" },
    { part: 2, starts: false, title: "" },
  ]);
});

test("pageMarks leaves pages outside every part unmarked", () => {
  const marks = pageMarks([range(2, 2), range(4, 4)], 5);
  assert.deepEqual(
    marks.map((m) => m.part),
    [null, 1, null, 2, null],
  );
});

test("pageMarks ignores pages past the document and lets a later part win an overlap", () => {
  const marks = pageMarks([range(0, 3), range(3, 9)], 4);
  assert.deepEqual(
    marks.map((m) => [m.part, m.starts]),
    [
      [1, true],
      [1, false],
      [2, true],
      [2, false],
    ],
  );
});

test("pageMarks marks an earlier part again where it resumes after an overlap", () => {
  const marks = pageMarks([range(1, 5, "Book"), range(3, 4, "Insert")], 5);
  assert.deepEqual(
    marks.map((m) => [m.part, m.starts, m.title]),
    [
      [1, true, "Book"],
      [1, false, ""],
      [2, true, "Insert"],
      [2, false, ""],
      [1, true, "Book"],
    ],
  );
});

test("pageMarks follows edits to the parts", () => {
  const before = pageMarks([range(1, 3), range(4, 6)], 6);
  const after = pageMarks([range(1, 2), range(3, 6)], 6);
  assert.equal(before[2].starts, false);
  assert.equal(after[2].starts, true);
  assert.equal(after[3].starts, false);
});

test("tileMark gives the class, badge, tooltip and label for a tile", () => {
  assert.deepEqual(tileMark(1, undefined), { className: undefined, badge: null, title: "", label: "Page 1" });
  assert.deepEqual(tileMark(2, { part: null, starts: false, title: "" }), {
    className: "left-out",
    badge: null,
    title: "",
    label: "Page 2, left out",
  });
  assert.deepEqual(tileMark(3, { part: 2, starts: true, title: "Results" }), {
    className: "starts-part",
    badge: "Part 2 · Results",
    title: "Results",
    label: "Page 3, starts part 2: Results",
  });
  assert.deepEqual(tileMark(3, { part: 2, starts: true, title: "" }), {
    className: "starts-part",
    badge: "Part 2",
    title: "",
    label: "Page 3, starts part 2",
  });
  assert.deepEqual(tileMark(4, { part: 2, starts: false, title: "" }), {
    className: undefined,
    badge: null,
    title: "",
    label: "Page 4, part 2",
  });
});

test("tileMark shows no badge on a left-out page, even one marked as a start", () => {
  const tile = tileMark(5, { part: null, starts: true, title: "" });
  assert.equal(tile.className, "left-out");
  assert.equal(tile.badge, null);
});
