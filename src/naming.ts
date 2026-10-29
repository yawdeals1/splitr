// Showing export file names (FR-51, FR-52). The names themselves come from the backend's
// `export_names`, so the preview and the export can't disagree.
import { baseName } from "./file.ts";

/** How many names the preview lists before "and N more". */
export const PREVIEW_COUNT = 3;

/** The first names to show, and how many are left over. */
export function namePreview(names: string[]): { shown: string[]; more: number } {
  return { shown: names.slice(0, PREVIEW_COUNT), more: Math.max(0, names.length - PREVIEW_COUNT) };
}

/** The message after an export, such as "Saved 3 files to D:\out." */
export function savedMessage(count: number, folder: string, renamed: number, zipFile?: string): string {
  const files = `${count} ${count === 1 ? "file" : "files"}`;
  if (zipFile) return `Saved ${files} in ${baseName(zipFile)} to ${folder}.`;
  const note =
    renamed === 0
      ? ""
      : ` ${renamed === 1 ? "One file" : `${renamed} files`} got a number added, because the folder already had a file with that name.`;
  return `Saved ${files} to ${folder}.${note}`;
}
