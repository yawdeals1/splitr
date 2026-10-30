// The blank page sensitivity setting (FR-32, SRS 6.4).

/** The most visible characters a page may have and still count as blank; matches settings.rs. */
export const MAX_BLANK_THRESHOLD = 200;

/** The threshold typed in the field, or null unless it's a whole number from 0 to the maximum. */
export function parseThreshold(text: string): number | null {
  const trimmed = text.trim();
  if (!/^\d+$/.test(trimmed)) return null;
  const value = Number(trimmed);
  return value <= MAX_BLANK_THRESHOLD ? value : null;
}
