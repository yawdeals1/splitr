// Unit tests for file.ts. Run with `npm test` (Node's built-in test runner).
import { test } from "node:test";
import assert from "node:assert/strict";
import { baseName, formatSize, isPdfPath, partNames, rejectReason } from "./file.ts";

test("isPdfPath accepts .pdf in any letter case", () => {
  assert.equal(isPdfPath("C:\\docs\\a.pdf"), true);
  assert.equal(isPdfPath("/home/u/REPORT.PDF"), true);
  assert.equal(isPdfPath("/home/u/notes.txt"), false);
  assert.equal(isPdfPath("/home/u/a.pdf.zip"), false);
});

test("baseName handles Windows and POSIX paths", () => {
  assert.equal(baseName("C:\\docs\\a.pdf"), "a.pdf");
  assert.equal(baseName("/home/u/b.pdf"), "b.pdf");
  assert.equal(baseName("/home/u/scans/"), "scans");
  assert.equal(baseName("plain.pdf"), "plain.pdf");
});

test("formatSize picks the unit after rounding", () => {
  assert.equal(formatSize(0), "0 bytes");
  assert.equal(formatSize(1023), "1023 bytes");
  assert.equal(formatSize(1024), "1.0 KB");
  assert.equal(formatSize(1536), "1.5 KB");
  assert.equal(formatSize(1_048_500), "1023.9 KB");
  assert.equal(formatSize(1_048_570), "1.0 MB");
  assert.equal(formatSize(1024 ** 3), "1.0 GB");
  assert.equal(formatSize(5 * 1024 ** 4), "5120.0 GB");
});

test("rejectReason explains what can't be opened", () => {
  assert.equal(rejectReason(["/a.pdf"]), null);
  assert.match(rejectReason([]) ?? "", /Drop a PDF file/);
  assert.equal(rejectReason(["/a.pdf", "/b.pdf"]), "Drop one PDF at a time.");
  assert.match(rejectReason(["/x/notes.txt"]) ?? "", /"notes.txt" is not a PDF/);
});

test("partNames numbers parts and adds titles", () => {
  assert.deepEqual(partNames("Book.PDF", ["", " Intro ", ""]), ["Book - 01", "Book - 02 - Intro", "Book - 03"]);
  assert.deepEqual(partNames("scan.pdf.pdf", [""]), ["scan.pdf - 01"]);
  assert.deepEqual(partNames("notes", []), []);
});
