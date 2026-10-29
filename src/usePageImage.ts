// Loads page images from the backend for the thumbnail grid and page view (FR-10, FR-11, NFR-03).
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Cancelled, createQueue } from "./thumbnails";

// Two renders at a time keeps the grid responsive; PDFium handles one page at a time anyway.
// The grid and the page view share this queue.
const schedule = createQueue(2);

/** The image of one page while `active`, as an object URL, or why it couldn't be rendered. */
export function usePageImage(path: string, page: number, width: number, active: boolean) {
  const [image, setImage] = useState<{ url?: string; error?: string }>({});

  useEffect(() => {
    if (!active) return;
    let alive = true;
    let url: string | undefined;
    const job = schedule(() => invoke<ArrayBuffer>("render_page", { path, page, width }));
    job.result.then(
      (bytes) => {
        if (!alive) return;
        url = URL.createObjectURL(new Blob([bytes], { type: "image/jpeg" }));
        setImage({ url });
      },
      (e) => alive && !(e instanceof Cancelled) && setImage({ error: String(e) }),
    );
    return () => {
      alive = false;
      job.cancel();
      if (url) URL.revokeObjectURL(url);
      setImage({});
    };
  }, [path, page, width, active]);

  return image;
}
