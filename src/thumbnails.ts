// Scheduling page renders for the thumbnail grid and page view (FR-10, FR-11, NFR-03).

/** Thrown by a cancelled task's result, so callers can ignore it. */
export class Cancelled extends Error {
  constructor() {
    super("Cancelled");
  }
}

export interface Scheduled<T> {
  result: Promise<T>;
  /**
   * Drops the task if it hasn't started. A started task can't be stopped: it frees its slot at once,
   * so the next task starts, and its result rejects with Cancelled.
   */
  cancel(): void;
}

/**
 * Runs at most `limit` tasks at a time, newest first. When the user scrolls fast, the pages now on
 * screen render before the ones they scrolled past, and pages scrolled out of view are cancelled.
 */
export function createQueue(limit: number) {
  const waiting: (() => void)[] = [];
  let running = 0;

  function next() {
    while (running < limit && waiting.length > 0) waiting.pop()!();
  }

  return function schedule<T>(task: () => Promise<T>): Scheduled<T> {
    let cancelled = false;
    let started = false;
    let released = false;
    let start!: () => void;
    let fail!: (reason: unknown) => void;

    // Frees this task's slot exactly once, whether it finished, failed or was cancelled.
    function release() {
      if (released) return;
      released = true;
      running--;
      next();
    }

    const result = new Promise<T>((resolve, reject) => {
      fail = reject;
      start = () => {
        started = true;
        running++;
        // `new Promise` also catches a task that throws before returning its promise.
        new Promise<T>((run) => run(task()))
          .then((value) => (cancelled ? reject(new Cancelled()) : resolve(value)), reject)
          .finally(release);
      };
    });
    waiting.push(start);
    next();
    return {
      result,
      cancel() {
        if (cancelled) return;
        cancelled = true;
        const i = waiting.indexOf(start);
        if (i >= 0) waiting.splice(i, 1);
        fail(new Cancelled());
        if (started) release();
      },
    };
  };
}

/**
 * The width in pixels to render a page shown `cssWidth` wide, rounded up to a step of 64 so that
 * small layout changes reuse the cached image instead of rendering the page again.
 */
export function renderWidth(cssWidth: number, pixelRatio: number): number {
  const step = 64;
  return Math.max(step, Math.ceil((cssWidth * pixelRatio) / step) * step);
}

/** Widest the page view shows a page, in CSS pixels. */
export const VIEW_WIDTH = 900;
/** Room the page view leaves around the page, in CSS pixels. */
export const VIEW_MARGIN = 96;
/** Narrowest the page view shows a page, in CSS pixels. */
export const VIEW_MIN_WIDTH = 160;

/** The CSS width of the page in the page view for a window `windowWidth` wide (FR-11). */
export function viewWidth(windowWidth: number): number {
  return Math.max(VIEW_MIN_WIDTH, Math.min(VIEW_WIDTH, windowWidth - VIEW_MARGIN));
}
