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

/** How a page tile shows its mark (FR-12). */
export interface TileMark {
  /** "left-out", "starts-part", or undefined for an ordinary page. */
  className: string | undefined;
  /** The badge on the first page of a part, such as "Part 2 · Results"; null on other pages. */
  badge: string | null;
  /** The part's title, shown as the badge's tooltip; "" for none. */
  title: string;
  /** What a screen reader hears, such as "Page 5, starts part 2: Results". */
  label: string;
}

/**
 * The class, badge and label for page `page` (FR-12). The class and the badge come from the same
 * branch, so a tile can't get the start-of-part edge without its badge, or a badge while left out.
 */
export function tileMark(page: number, mark: PageMark | undefined): TileMark {
  const plain = { className: undefined, badge: null, title: "" };
  if (!mark) return { ...plain, label: `Page ${page}` };
  if (mark.part === null) return { ...plain, className: "left-out", label: `Page ${page}, left out` };
  if (!mark.starts) return { ...plain, label: `Page ${page}, part ${mark.part}` };
  const titled = (separator: string) => (mark.title ? `${separator}${mark.title}` : "");
  return {
    className: "starts-part",
    badge: `Part ${mark.part}${titled(" · ")}`,
    title: mark.title,
    label: `Page ${page}, starts part ${mark.part}${titled(": ")}`,
  };
}
