// Lazy thumbnail grid (FR-10, NFR-03). Only pages near the visible area are rendered and kept in
// memory; pages scrolled far away release their image, so a 1,000-page file stays light.
import { memo, useEffect, useRef, useState } from "react";
import { renderWidth } from "./thumbnails";
import { usePageImage } from "./usePageImage";

/** Thumbnail width in CSS pixels; must match `.page-grid` in App.css. */
const THUMB_WIDTH = 140;
/** Pages within this distance of the visible area render ahead of scrolling. */
const PREFETCH_MARGIN = "600px 0px";

interface Props {
  path: string;
  pageCount: number;
  /** Opens the larger page view (FR-11). */
  onOpen: (page: number) => void;
}

export function PageGrid({ path, pageCount, onOpen }: Props) {
  const grid = useRef<HTMLOListElement>(null);
  const [observer, setObserver] = useState<IntersectionObserver | null>(null);
  const width = renderWidth(THUMB_WIDTH, window.devicePixelRatio || 1);

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
          <Thumbnail key={i} path={path} page={i + 1} width={width} observer={observer} onOpen={onOpen} />
        ))}
    </ol>
  );
}

const onVisible = new WeakMap<Element, (visible: boolean) => void>();

interface ThumbnailProps {
  path: string;
  page: number;
  width: number;
  observer: IntersectionObserver;
  onOpen: (page: number) => void;
}

const Thumbnail = memo(function Thumbnail({ path, page, width, observer, onOpen }: ThumbnailProps) {
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
    <li ref={tile}>
      <button type="button" onClick={() => onOpen(page)} aria-label={`Page ${page}`} title={error}>
        <span className="page-image">
          {url ? <img src={url} alt="" draggable={false} /> : error && <span className="page-error">!</span>}
        </span>
        <span className="page-number">{page}</span>
      </button>
    </li>
  );
});
