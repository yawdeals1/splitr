// Scheduling page renders for the thumbnail grid and page view (FR-10, FR-11, NFR-03).

/** Thrown by a cancelled task's result, so callers can ignore it. */
export class Cancelled extends Error {
  constructor() {
    super("Cancelled");
  }
}

export interface Scheduled<T> {
  result: Promise<T>;
  /** Drops the task if it hasn't started. A started task finishes, but its result rejects with Cancelled. */
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
    let start!: () => void;
    let fail!: (reason: unknown) => void;
    const result = new Promise<T>((resolve, reject) => {
      fail = reject;
      start = () => {
        running++;
        task()
          .then((value) => (cancelled ? reject(new Cancelled()) : resolve(value)), reject)
          .finally(() => {
            running--;
            next();
          });
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
