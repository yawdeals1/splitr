// Keyboard movement in the thumbnail grid (NFR-11). The grid is one Tab stop; arrow keys move
// between pages, so the split options stay a few Tab presses away however long the PDF is.

/**
 * The page that `key` moves to from `page` in a grid `columns` wide, or null if the key doesn't
 * move. Moves stop at the first and last page; ArrowDown into a short last row goes to the last page.
 */
export function gridMove(key: string, page: number, pageCount: number, columns: number): number | null {
  let to: number;
  switch (key) {
    case "Home":
      return 1;
    case "End":
      return pageCount;
    case "ArrowLeft":
      to = page - 1;
      break;
    case "ArrowRight":
      to = page + 1;
      break;
    case "ArrowUp":
      to = page - columns;
      break;
    case "ArrowDown":
      to = page + columns;
      // A row below exists but is too short to have a page under this one.
      if (to > pageCount && Math.ceil(page / columns) < Math.ceil(pageCount / columns)) return pageCount;
      break;
    default:
      return null;
  }
  return to < 1 || to > pageCount ? page : to;
}

/** The number of columns in a CSS grid, from its computed `grid-template-columns` ("140px 140px ..."). */
export function columnCount(templateColumns: string): number {
  return Math.max(1, templateColumns.trim().split(/\s+/).filter(Boolean).length);
}
