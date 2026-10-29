// Edits on the review screen (FR-41). Each returns a new list; the backend validates it (FR-35).
import type { Range } from "./types";

const replace = (ranges: Range[], i: number, part: Range) => ranges.map((r, j) => (j === i ? part : r));

/** Moves the start of part `i`. A previous part that touched it moves its end along. */
export function setStart(ranges: Range[], i: number, page: number): Range[] {
  const next = replace(ranges, i, { ...ranges[i], start: page });
  const prev = ranges[i - 1];
  if (prev && prev.end === ranges[i].start - 1) next[i - 1] = { ...prev, end: page - 1 };
  return next;
}

/** Moves the end of part `i`. A following part that touched it moves its start along. */
export function setEnd(ranges: Range[], i: number, page: number): Range[] {
  const next = replace(ranges, i, { ...ranges[i], end: page });
  const after = ranges[i + 1];
  if (after && after.start === ranges[i].end + 1) next[i + 1] = { ...after, start: page + 1 };
  return next;
}

/** Splits part `i` so `page` starts a new part, or returns null if `page` isn't inside it past its first page. */
export function splitAt(ranges: Range[], i: number, page: number): Range[] | null {
  const part = ranges[i];
  if (!Number.isInteger(page) || page <= part.start || page > part.end) return null;
  return [...ranges.slice(0, i), { ...part, end: page - 1 }, { start: page, end: part.end, title: "" }, ...ranges.slice(i + 1)];
}

/** Removes the split after part `i`, joining it with the next part and keeping the first title. */
export function mergeWithNext(ranges: Range[], i: number): Range[] {
  const after = ranges[i + 1];
  if (!after) return ranges;
  return [...ranges.slice(0, i), { ...ranges[i], end: after.end }, ...ranges.slice(i + 2)];
}

/** Drops part `i`; its pages are left out of the export. */
export function removePart(ranges: Range[], i: number): Range[] {
  return ranges.filter((_, j) => j !== i);
}

export function rename(ranges: Range[], i: number, title: string): Range[] {
  return replace(ranges, i, { ...ranges[i], title });
}

/** Pages a part covers, or 0 while its bounds are invalid. */
export function pageCount(r: Range): number {
  return r.end >= r.start ? r.end - r.start + 1 : 0;
}

/** How many pages of the document no part covers. */
export function uncoveredPages(ranges: Range[], total: number): number {
  const covered = new Set<number>();
  for (const r of ranges) for (let p = Math.max(1, r.start); p <= Math.min(total, r.end); p++) covered.add(p);
  return total - covered.size;
}
