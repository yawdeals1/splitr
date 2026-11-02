#!/usr/bin/env node
// Fails the release build when an installer is larger than 25 MB (NFR-13, #21).
//
// Usage: node scripts/check-installer-size.mjs '<JSON array of paths>'
// The release workflow passes tauri-action's `artifactPaths` output. Update signatures and
// latest.json are not installers and are skipped; so is anything that isn't a file.
// The AppImage is reported but can't fail the check: SRS 8 has it bundle what it needs, WebKitGTK
// included, which alone is larger than 25 MB. The .deb uses the system's WebKitGTK and is checked.

import { realpathSync, statSync } from "node:fs";
import { basename } from "node:path";
import { fileURLToPath } from "node:url";

/** NFR-13: the largest installer allowed, in bytes. */
export const LIMIT = 25 * 1024 * 1024;

/** File endings of installers and update bundles that users download. */
const INSTALLERS = [".msi", ".exe", ".dmg", ".appimage", ".deb", ".rpm", ".app.tar.gz", ".msi.zip", ".nsis.zip"];

export function isInstaller(path) {
  const name = basename(path).toLowerCase();
  return INSTALLERS.some((end) => name.endsWith(end));
}

/** Installers that bundle their own web engine (SRS 8), so NFR-13 can't apply to them. */
export function bundlesWebEngine(path) {
  return basename(path).toLowerCase().endsWith(".appimage");
}

/** "12.3 MB" */
export function megabytes(bytes) {
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

/** One line per installer, and the ones over the limit. `sizes` maps path to size in bytes. */
export function report(sizes) {
  const lines = [];
  const tooBig = [];
  for (const [path, bytes] of sizes) {
    if (!isInstaller(path)) continue;
    const over = bytes > LIMIT;
    const exempt = over && bundlesWebEngine(path);
    const verdict = exempt ? "note" : over ? "TOO BIG" : "ok";
    const why = exempt ? "  (bundles WebKitGTK, SRS 8)" : "";
    lines.push(`${verdict.padEnd(7)}  ${megabytes(bytes).padStart(8)}  ${basename(path)}${why}`);
    if (over && !exempt) tooBig.push(path);
  }
  return { lines, tooBig };
}

function main() {
  const paths = JSON.parse(process.argv[2] ?? "[]");
  const sizes = [];
  for (const path of paths) {
    try {
      const stat = statSync(path);
      if (stat.isFile()) sizes.push([path, stat.size]);
    } catch {
      // A path tauri-action listed but didn't write isn't an installer to check.
    }
  }
  const { lines, tooBig } = report(sizes);
  if (lines.length === 0) {
    console.error("No installers found to check.");
    process.exit(1);
  }
  for (const line of lines) console.log(line);
  if (tooBig.length > 0) {
    console.error(`${tooBig.length} installer(s) are over the ${megabytes(LIMIT)} limit (NFR-13).`);
    process.exit(1);
  }
}

// Real paths, because a mismatch here would skip the check and pass the build.
if (process.argv[1] && realpathSync(process.argv[1]) === realpathSync(fileURLToPath(import.meta.url))) main();
