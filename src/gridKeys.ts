// Keyboard movement in the thumbnail grid (NFR-11). The grid is one Tab stop; arrow keys move
// between pages, so the split options stay a few Tab presses away however long the PDF is.

/**
 * The page that `key` moves to from `page` in a grid `columns` wide, or null if the key doesn't
 * move. Moves stop at the first and last page.
 */
export function gridMove(key: string, page: number, pageCount: number, columns: number): number | null {
  const step: Record<string, number> = {
    ArrowLeft: -1,
    ArrowRight: 1,
    ArrowUp: -columns,
    ArrowDown: columns,
  };
  if (key === "Home") return 1;
  if (key === "End") return pageCount;
  if (!(key in step)) return null;
  const to = page + step[key];
  return to < 1 || to > pageCount ? page : to;
}

/** The number of columns in a CSS grid, from its computed `grid-template-columns` ("140px 140px ..."). */
export function columnCount(templateColumns: string): number {
  return Math.max(1, templateColumns.trim().split(/\s+/).filter(Boolean).length);
}
