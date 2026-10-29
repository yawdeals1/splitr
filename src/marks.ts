// Split marks on the thumbnail grid (FR-12): where each part starts, and which pages are left out.
import type { Range } from "./types";

export interface PageMark {
  /** The 1-based part this page belongs to, or null if no part includes it. */
  part: number | null;
  /** True on the first page of a part. */
  starts: boolean;
  /** The part's title where the part starts, otherwise "". */
  title: string;
}

/**
 * One mark per page, from page 1 to `pageCount`, for the parts on the review screen. An empty list
 * means there are no parts yet, so nothing is marked. Pages outside 1 to `pageCount` are ignored.
 * If parts overlap while being edited, the later part wins; a page starts a part whenever its part
 * differs from the page before, so an earlier part that resumes after the overlap is marked again.
 */
export function pageMarks(ranges: Range[], pageCount: number): PageMark[] {
  if (ranges.length === 0) return [];
  const parts: (number | null)[] = Array(pageCount).fill(null);
  ranges.forEach((r, i) => {
    for (let page = Math.max(1, r.start); page <= Math.min(pageCount, r.end); page++) parts[page - 1] = i + 1;
  });
  return parts.map((part, i) => {
    const starts = part !== null && (i === 0 || parts[i - 1] !== part);
    return { part, starts, title: starts ? ranges[part - 1].title.trim() : "" };
  });
}

/** What a screen reader hears for a page tile, such as "Page 5, starts part 2: Results". */
export function pageLabel(page: number, mark: PageMark | undefined): string {
  if (!mark) return `Page ${page}`;
  if (mark.part === null) return `Page ${page}, left out`;
  if (mark.starts) return `Page ${page}, starts part ${mark.part}${mark.title ? `: ${mark.title}` : ""}`;
  return `Page ${page}, part ${mark.part}`;
}
