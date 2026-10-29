// Lazy thumbnail grid (FR-10, NFR-03). Only pages near the visible area are rendered and kept in
// memory; pages scrolled far away release their image, so a 1,000-page file stays light.
// Once there are parts, each tile shows where a part starts and which pages are left out (FR-12).
// The grid is one Tab stop and the arrow keys move between pages (NFR-11).
import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { columnCount, gridMove } from "./gridKeys";
import { pageMarks, tileMark, type TileMark } from "./marks";
import { renderWidth } from "./thumbnails";
import type { Range } from "./types";
import { usePageImage } from "./usePageImage";

/** Thumbnail width in CSS pixels; must match `.page-grid` in App.css. */
const THUMB_WIDTH = 140;
/** Pages within this distance of the visible area render ahead of scrolling. */
const PREFETCH_MARGIN = "600px 0px";
/** The keys that move between tiles (NFR-11). */
const GRID_KEYS = "ArrowLeft ArrowRight ArrowUp ArrowDown Home End";

/** The page number of the tile button `target`, or null if it isn't one. */
function tilePage(target: EventTarget): number | null {
  const page = target instanceof HTMLElement ? target.dataset.page : undefined;
  return page ? Number(page) : null;
}

interface Props {
  path: string;
  pageCount: number;
  /** The parts on the review screen, empty before a split. Edits there update the marks (FR-41). */
  ranges: Range[];
  /** Opens the larger page view (FR-11). */
  onOpen: (page: number) => void;
}

export function PageGrid({ path, pageCount, ranges, onOpen }: Props) {
  const grid = useRef<HTMLOListElement>(null);
  const [observer, setObserver] = useState<IntersectionObserver | null>(null);
  const width = renderWidth(THUMB_WIDTH, window.devicePixelRatio || 1);
  // Built only when the parts change, not on every render (a 1,000-page file has 1,000 tiles).
  const tiles = useMemo(() => {
    const marks = pageMarks(ranges, pageCount);
    return Array.from({ length: pageCount }, (_, i) => tileMark(i + 1, marks[i]));
  }, [ranges, pageCount]);
  // The one tile reached by Tab: the last page moved to or opened.
  const [active, setActive] = useState(1);

  // One observer for every tile; each tile registers its own callback in `onVisible`.
  useEffect(() => {
    const io = new IntersectionObserver(
      (entries) => entries.forEach((e) => onVisible.get(e.target)?.(e.isIntersecting)),
      { root: grid.current, rootMargin: PREFETCH_MARGIN },
    );
    setObserver(io);
    return () => io.disconnect();
  }, []);

  // Moves start from the tile that has focus, which may not be `active` (a screen reader can move focus).
  function onKeyDown(e: React.KeyboardEvent) {
    const el = grid.current;
    const from = tilePage(e.target);
    if (!el || from === null || e.altKey || e.ctrlKey || e.metaKey) return;
    const to = gridMove(e.key, from, pageCount, columnCount(getComputedStyle(el).gridTemplateColumns));
    if (to === null) return;
    e.preventDefault();
    setActive(to);
    el.querySelector<HTMLElement>(`[data-page="${to}"]`)?.focus();
  }

  // Stable, so opening a page doesn't re-render every tile.
  const open = useCallback(
    (page: number) => {
      setActive(page);
      onOpen(page);
    },
    [onOpen],
  );

  return (
    <ol
      className="page-grid"
      ref={grid}
      aria-label="Pages"
      onKeyDown={onKeyDown}
      // Whichever tile gets focus, by click, keys or a screen reader, becomes the one Tab returns to.
      onFocus={(e) => {
        const page = tilePage(e.target);
        if (page !== null) setActive(page);
      }}
    >
      {observer &&
        tiles.map((tile, i) => (
          <Thumbnail
            key={i}
            path={path}
            page={i + 1}
            width={width}
            observer={observer}
            tabbable={i + 1 === active}
            onOpen={open}
            // Spread as plain values, so an edit re-renders only the tiles whose mark changed.
            {...tile}
          />
        ))}
    </ol>
  );
}

const onVisible = new WeakMap<Element, (visible: boolean) => void>();

interface ThumbnailProps extends TileMark {
  path: string;
  page: number;
  width: number;
  observer: IntersectionObserver;
  /** True for the one tile in the Tab order. */
  tabbable: boolean;
  onOpen: (page: number) => void;
}

const Thumbnail = memo(function Thumbnail({ path, page, width, observer, tabbable, onOpen, className, badge, title, label }: ThumbnailProps) {
  const tile = useRef<HTMLLIElement>(null);
  const [visible, setVisible] = useState(false);
  const { url, error } = usePageImage(path, page, width, visible);

  useEffect(() => {
    const el = tile.current;
    if (!el) return;
    onVisible.set(el, setVisible);
    observer.observe(el);
    return () => {
      observer.unobserve(el);
      onVisible.delete(el);
    };
  }, [observer]);

  return (
    <li ref={tile} className={className}>
      <button
        type="button"
        data-page={page}
        tabIndex={tabbable ? 0 : -1}
        onClick={() => onOpen(page)}
        aria-label={label}
        // On the tile, not the list: assistive tech announces shortcuts of the focused element.
        aria-keyshortcuts={GRID_KEYS}
        title={error}
      >
        <span className="page-image">
          {url ? <img src={url} alt="" draggable={false} /> : error && <span className="page-error">!</span>}
          {badge !== null && (
            <span className="part-badge" aria-hidden="true" title={title || undefined}>
              {badge}
            </span>
          )}
        </span>
        <span className="page-number">{page}</span>
      </button>
    </li>
  );
});
