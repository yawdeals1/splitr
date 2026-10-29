import { describe, expect, it } from "vitest";
import { mergeWithNext, pageCount, removePart, rename, setEnd, setStart, splitAt, uncoveredPages } from "./review";
import type { Range } from "./types";

const r = (start: number, end: number, title = ""): Range => ({ start, end, title });
const parts = [r(1, 4, "A"), r(5, 9, "B"), r(10, 12, "C")];

describe("review edits", () => {
  it("moves a shared boundary from either side", () => {
    expect(setEnd(parts, 0, 6)).toEqual([r(1, 6, "A"), r(7, 9, "B"), r(10, 12, "C")]);
    expect(setStart(parts, 2, 8)).toEqual([r(1, 4, "A"), r(5, 7, "B"), r(8, 12, "C")]);
  });

  it("leaves a neighbour alone when there is a gap", () => {
    const gapped = [r(1, 3), r(6, 9)];
    expect(setEnd(gapped, 0, 4)).toEqual([r(1, 4), r(6, 9)]);
    expect(setStart(gapped, 1, 5)).toEqual([r(1, 3), r(5, 9)]);
  });

  it("does not mutate its input", () => {
    const copy = structuredClone(parts);
    setEnd(parts, 0, 6);
    splitAt(parts, 1, 7);
    mergeWithNext(parts, 0);
    removePart(parts, 1);
    rename(parts, 0, "Z");
    expect(parts).toEqual(copy);
  });

  it("adds a split inside a part", () => {
    expect(splitAt(parts, 1, 7)).toEqual([r(1, 4, "A"), r(5, 6, "B"), r(7, 9), r(10, 12, "C")]);
    expect(splitAt(parts, 1, 9)).toEqual([r(1, 4, "A"), r(5, 8, "B"), r(9, 9), r(10, 12, "C")]);
  });

  it("refuses a split outside the part or on its first page", () => {
    expect(splitAt(parts, 1, 5)).toBeNull();
    expect(splitAt(parts, 1, 10)).toBeNull();
    expect(splitAt(parts, 1, 6.5)).toBeNull();
  });

  it("removes a split by merging with the next part", () => {
    expect(mergeWithNext(parts, 0)).toEqual([r(1, 9, "A"), r(10, 12, "C")]);
    expect(mergeWithNext(parts, 2)).toBe(parts);
  });

  it("deletes and renames parts", () => {
    expect(removePart(parts, 1)).toEqual([r(1, 4, "A"), r(10, 12, "C")]);
    expect(rename(parts, 2, "Appendix")[2]).toEqual(r(10, 12, "Appendix"));
  });

  it("counts pages", () => {
    expect(pageCount(r(5, 9))).toBe(5);
    expect(pageCount(r(9, 5))).toBe(0);
    expect(uncoveredPages(parts, 12)).toBe(0);
    expect(uncoveredPages(removePart(parts, 1), 12)).toBe(5);
  });
});
