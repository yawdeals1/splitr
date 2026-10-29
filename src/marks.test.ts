// Unit tests for marks.ts. Run with `npm test` (Node's built-in test runner).
import { test } from "node:test";
import assert from "node:assert/strict";
import { pageLabel, pageMarks } from "./marks.ts";

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

test("pageLabel describes the mark", () => {
  assert.equal(pageLabel(3, undefined), "Page 3");
  assert.equal(pageLabel(3, { part: null, starts: false, title: "" }), "Page 3, left out");
  assert.equal(pageLabel(3, { part: 2, starts: true, title: "Results" }), "Page 3, starts part 2: Results");
  assert.equal(pageLabel(3, { part: 2, starts: true, title: "" }), "Page 3, starts part 2");
  assert.equal(pageLabel(4, { part: 2, starts: false, title: "" }), "Page 4, part 2");
});
