// Helpers for opening a file (FR-01, FR-02).

/** True when the path ends in .pdf, in any letter case. */
export function isPdfPath(path: string): boolean {
  return /\.pdf$/i.test(path);
}

/** Last segment of a Windows or POSIX path, ignoring trailing separators. */
export function baseName(path: string): string {
  return path.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || path;
}

/** Human-readable size such as "1.4 MB" (FR-02). */
export function formatSize(bytes: number): string {
  const units = ["bytes", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  // Compare the value as it will be shown, so 1,048,570 bytes is "1.0 MB", not "1024.0 KB".
  const shown = () => (unit === 0 ? value : Math.round(value * 10) / 10);
  while (shown() >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  return unit === 0 ? `${value} ${units[0]}` : `${value.toFixed(1)} ${units[unit]}`;
}

/** The message to show when a dropped or picked path can't be opened, or null if it can. */
export function rejectReason(paths: string[]): string | null {
  if (paths.length === 0) return "Drop a PDF file. Text and links can't be opened.";
  if (paths.length > 1) return "Drop one PDF at a time.";
  if (!isPdfPath(paths[0])) return `"${baseName(paths[0])}" is not a PDF. Choose a file that ends in .pdf.`;
  return null;
}
