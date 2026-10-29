// Larger view of one page (FR-11). Arrow keys move between pages; Escape closes (NFR-11).
import { useEffect, useRef, useState } from "react";
import { renderWidth, viewWidth } from "./thumbnails";
import { usePageImage } from "./usePageImage";

/** The page's CSS width, following the window size (VIEW_MARGIN matches `.page-viewer` in App.css). */
function useViewWidth() {
  const [width, setWidth] = useState(() => viewWidth(window.innerWidth));
  useEffect(() => {
    const onResize = () => setWidth(viewWidth(window.innerWidth));
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);
  return width;
}

interface Props {
  path: string;
  page: number;
  pageCount: number;
  onPage: (page: number) => void;
  onClose: () => void;
}

export function PageViewer({ path, page, pageCount, onPage, onClose }: Props) {
  const dialog = useRef<HTMLDialogElement>(null);
  const cssWidth = useViewWidth();
  const { url, error } = usePageImage(path, page, renderWidth(cssWidth, window.devicePixelRatio || 1), true);
  const go = (to: number) => onPage(Math.min(pageCount, Math.max(1, to)));

  // A modal <dialog> keeps focus inside and closes on Escape.
  useEffect(() => {
    dialog.current?.showModal();
  }, []);

  return (
    <dialog
      ref={dialog}
      className="page-viewer"
      aria-label={`Page ${page} of ${pageCount}`}
      onClose={onClose}
      onKeyDown={(e) => {
        const to = { ArrowLeft: page - 1, ArrowRight: page + 1, Home: 1, End: pageCount }[e.key];
        if (to !== undefined) {
          e.preventDefault();
          go(to);
        }
      }}
    >
      <header>
        <button type="button" onClick={() => go(page - 1)} disabled={page <= 1} aria-label="Previous page">
          ‹
        </button>
        <span aria-live="polite">
          Page {page} of {pageCount}
        </span>
        <button type="button" onClick={() => go(page + 1)} disabled={page >= pageCount} aria-label="Next page">
          ›
        </button>
        <button type="button" className="close" onClick={() => dialog.current?.close()} autoFocus>
          Close
        </button>
      </header>
      <div className="page-view" style={{ width: cssWidth }}>
        {url ? (
          <img src={url} alt={`Page ${page}`} />
        ) : (
          <p className={error ? "error" : undefined}>{error ?? `Loading page ${page}…`}</p>
        )}
      </div>
    </dialog>
  );
}
