// Split marks on the thumbnail grid (FR-12): where each part starts, and which pages are left out.
import type { Range } from "./types";

export interface PageMark {
  /** The 1-based part this page belongs to, or null if no part includes it. */
  part: number | null;
  /** True on the first page of a part. */
  starts: boolean;
  /** The part's title on its first page, otherwise "". */
  title: string;
}

/**
 * One mark per page, from page 1 to `pageCount`, for the parts on the review screen. An empty list
 * means there are no parts yet, so nothing is marked. Pages outside 1 to `pageCount` are ignored,
 * and if parts overlap while being edited, the later part wins.
 */
export function pageMarks(ranges: Range[], pageCount: number): PageMark[] {
  if (ranges.length === 0) return [];
  const marks: PageMark[] = Array.from({ length: pageCount }, () => ({ part: null, starts: false, title: "" }));
  ranges.forEach((r, i) => {
    const first = Math.max(1, r.start);
    const last = Math.min(pageCount, r.end);
    for (let page = first; page <= last; page++) {
      marks[page - 1] = { part: i + 1, starts: page === r.start, title: page === r.start ? r.title.trim() : "" };
    }
  });
  return marks;
}

/** What a screen reader hears for a page tile, such as "Page 5, starts part 2: Results". */
export function pageLabel(page: number, mark: PageMark | undefined): string {
  if (!mark) return `Page ${page}`;
  if (mark.part === null) return `Page ${page}, left out`;
  if (mark.starts) return `Page ${page}, starts part ${mark.part}${mark.title ? `: ${mark.title}` : ""}`;
  return `Page ${page}, part ${mark.part}`;
}
