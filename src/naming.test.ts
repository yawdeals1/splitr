// Unit tests for naming.ts. Run with `npm test`.
import { test } from "node:test";
import assert from "node:assert/strict";
import { namePreview, renamedCount, savedMessage, zipName } from "./naming.ts";

test("namePreview lists the first names and counts the rest", () => {
  assert.deepEqual(namePreview(["a", "b"]), { shown: ["a", "b"], more: 0 });
  assert.deepEqual(namePreview(["a", "b", "c", "d", "e"]), { shown: ["a", "b", "c"], more: 2 });
  assert.deepEqual(namePreview([]), { shown: [], more: 0 });
});

test("zipName drops the .pdf extension in any letter case", () => {
  assert.equal(zipName("Book.PDF"), "Book");
  assert.equal(zipName("scan.pdf.pdf"), "scan.pdf");
  assert.equal(zipName("notes"), "notes");
});

test("renamedCount finds files saved under another name", () => {
  const names = ["Book - 01", "Book - 02"];
  assert.equal(renamedCount(names, ["D:\\out\\Book - 01.pdf", "D:\\out\\Book - 02.pdf"]), 0);
  assert.equal(renamedCount(names, ["/out/Book - 01 (2).pdf", "/out/book - 02.PDF"]), 1);
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
