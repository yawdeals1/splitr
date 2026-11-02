// Edits on the review screen (FR-41). Each returns a new list; the backend validates it (FR-35).
import type { Range } from "./types";

/** A run of pages, first to last, both included. */
export interface Span {
  start: number;
  end: number;
}

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

/** True when part `i` ends right before the next part starts, so joining them adds no pages. */
export function canJoin(ranges: Range[], i: number): boolean {
  const after = ranges[i + 1];
  return !!after && after.start === ranges[i].end + 1;
}

/**
 * Removes the split after part `i`, joining it with the next part and keeping the first title.
 * Parts with pages left out between them are not joined, so those pages never come back silently.
 */
export function mergeWithNext(ranges: Range[], i: number): Range[] {
  if (!canJoin(ranges, i)) return ranges;
  return [...ranges.slice(0, i), { ...ranges[i], end: ranges[i + 1].end }, ...ranges.slice(i + 2)];
}

/** Drops part `i`; its pages are left out of the export. */
export function removePart(ranges: Range[], i: number): Range[] {
  return ranges.filter((_, j) => j !== i);
}

/** An edit on the review screen that removes or disables the control that had focus. */
export type FocusEdit =
  | { kind: "split"; part: number }
  | { kind: "join"; part: number }
  | { kind: "delete"; part: number; parts: number }
  | { kind: "close-split"; part: number };

/**
 * NFR-11: the `data-focus` value of the control that takes focus after `edit`, so focus never
 * drops to the page. `parts` is the number of parts before a delete.
 */
export function focusAfter(edit: FocusEdit): string {
  switch (edit.kind) {
    case "split":
      return `title-${edit.part + 1}`;
    case "join":
      return `title-${edit.part}`;
    case "delete":
      return `title-${Math.min(edit.part, edit.parts - 2)}`;
    case "close-split":
      return `add-split-${edit.part}`;
  }
}

export function rename(ranges: Range[], i: number, title: string): Range[] {
  return replace(ranges, i, { ...ranges[i], title });
}

/** Pages a part covers, or 0 while its bounds are invalid. */
export function pageCount(r: Span): number {
  return r.end >= r.start ? r.end - r.start + 1 : 0;
}

/** The page number typed in a page field, or null unless it's a whole page from 1 to `total`. */
export function parsePage(text: string, total: number): number | null {
  if (!/^\s*\d+\s*$/.test(text)) return null;
  const page = Number(text);
  return page >= 1 && page <= total ? page : null;
}

/** True if `count` can be sent as the number of pages or parts in Fixed mode: a whole number the backend can hold. */
export function validCount(count: number): boolean {
  return Number.isInteger(count) && count >= 1 && count <= 0xffffffff;
}

/** Sorted, non-overlapping spans covering the same pages as `spans`, clamped to 1..`total`. */
function union(spans: Span[], total: number): Span[] {
  const sorted = spans
    .map((s) => ({ start: Math.max(1, s.start), end: Math.min(total, s.end) }))
    .filter((s) => s.end >= s.start)
    .sort((a, b) => a.start - b.start);
  const out: Span[] = [];
  for (const s of sorted) {
    const last = out[out.length - 1];
    if (last && s.start <= last.end + 1) last.end = Math.max(last.end, s.end);
    else out.push({ ...s });
  }
  return out;
}

/** The runs of pages no part covers. Works on spans, not pages, so large files stay fast. */
export function gaps(ranges: Span[], total: number): Span[] {
  const out: Span[] = [];
  let next = 1;
  for (const s of union(ranges, total)) {
    if (s.start > next) out.push({ start: next, end: s.start - 1 });
    next = s.end + 1;
  }
  if (next <= total) out.push({ start: next, end: total });
  return out;
}

/** How many pages of the document no part covers. */
export function uncoveredPages(ranges: Span[], total: number): number {
  return gaps(ranges, total).reduce((n, g) => n + pageCount(g), 0);
}

/** True when every page in `inner` is also in `outer`. */
export function within(inner: Span[], outer: Span[], total: number): boolean {
  const allowed = union(outer, total);
  return union(inner, total).every((s) => allowed.some((a) => a.start <= s.start && s.end <= a.end));
}
