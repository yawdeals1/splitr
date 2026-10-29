// Lazy thumbnail grid (FR-10, NFR-03). Only pages near the visible area are rendered and kept in
// memory; pages scrolled far away release their image, so a 1,000-page file stays light.
// Once there are parts, each tile shows where a part starts and which pages are left out (FR-12).
import { memo, useEffect, useMemo, useRef, useState } from "react";
import { pageMarks, tileMark, type TileMark } from "./marks";
import { renderWidth } from "./thumbnails";
import type { Range } from "./types";
import { usePageImage } from "./usePageImage";

/** Thumbnail width in CSS pixels; must match `.page-grid` in App.css. */
const THUMB_WIDTH = 140;
/** Pages within this distance of the visible area render ahead of scrolling. */
const PREFETCH_MARGIN = "600px 0px";

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
  const marks = useMemo(() => pageMarks(ranges, pageCount), [ranges, pageCount]);

  // One observer for every tile; each tile registers its own callback in `onVisible`.
  useEffect(() => {
    const io = new IntersectionObserver(
      (entries) => entries.forEach((e) => onVisible.get(e.target)?.(e.isIntersecting)),
      { root: grid.current, rootMargin: PREFETCH_MARGIN },
    );
    setObserver(io);
    return () => io.disconnect();
  }, []);

  return (
    <ol className="page-grid" ref={grid} aria-label="Pages">
      {observer &&
        Array.from({ length: pageCount }, (_, i) => (
          <Thumbnail
            key={i}
            path={path}
            page={i + 1}
            width={width}
            observer={observer}
            onOpen={onOpen}
            // Spread as plain values, so an edit re-renders only the tiles whose mark changed.
            {...tileMark(i + 1, marks[i])}
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
  onOpen: (page: number) => void;
}

const Thumbnail = memo(function Thumbnail({ path, page, width, observer, onOpen, className, badge, label }: ThumbnailProps) {
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
      <button type="button" onClick={() => onOpen(page)} aria-label={label} title={error}>
        <span className="page-image">
          {url ? <img src={url} alt="" draggable={false} /> : error && <span className="page-error">!</span>}
          {badge && (
            <span className="part-badge" aria-hidden="true" title={badge}>
              {badge}
            </span>
          )}
        </span>
        <span className="page-number">{page}</span>
      </button>
    </li>
  );
});
