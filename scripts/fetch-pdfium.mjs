#!/usr/bin/env node
// Downloads the PDFium library for one platform into src-tauri/pdfium/ (SRS 2.5, 5.3, NFR-12).
// Tauri bundles that folder as a resource and pdf.rs loads the library from it at startup.
// The binaries come from bblanchon/pdfium-binaries and are never committed.
//
// Usage: node scripts/fetch-pdfium.mjs [--target <rust target triple>]
// Without --target it uses TAURI_ENV_TARGET_TRIPLE (set by `tauri dev` and `tauri build`),
// then the host platform. It skips the download when the right version is already in place.

import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { download } from "./download.mjs";

// Must match the pdfium_NNNN feature of pdfium-render in src-tauri/Cargo.toml.
const VERSION = "chromium/7881";

// SHA-256 of each archive, from the GitHub release.
const ARCHIVES = {
  "win-x64": "73cc0de638ac2095e7445bf56a38200a5b7c7ca0e9f4ba144598f2457377ac08",
  "win-arm64": "d3035d4d2cacac6ecd1a2ece197a3d702a1b2a58466276b9f870b8cb278a9d84",
  "mac-x64": "6dedf83990e0e3d6b7c93c9e7589c5a126b0ae14b7464d76120cff7a26afb18b",
  "mac-arm64": "52e94ca5aa8847934330daf3f8150c190682c5ca93831468794f8b90d4392e40",
  "linux-x64": "1470e21b8b4a3b4ad7f85684e2da11d94f3b69a86d81dee11b9b6709d927ac1d",
  "linux-arm64": "ee7f7b7d5468958336a818c1cd580bdd20972846b7377b13f9a923d92d1d4674",
};

// Where the library sits inside each archive, and its file name.
const LIBRARY = {
  win: { from: "bin/pdfium.dll", name: "pdfium.dll" },
  mac: { from: "lib/libpdfium.dylib", name: "libpdfium.dylib" },
  linux: { from: "lib/libpdfium.so", name: "libpdfium.so" },
};

const outDir = join(dirname(fileURLToPath(import.meta.url)), "..", "src-tauri", "pdfium");

/** Maps a Rust target triple, or the host when none is given, to an archive name such as "win-x64". */
function platformFor(triple) {
  if (triple) {
    const os = triple.includes("windows") ? "win" : triple.includes("apple") ? "mac" : triple.includes("linux") ? "linux" : null;
    const arch = triple.startsWith("x86_64") ? "x64" : triple.startsWith("aarch64") ? "arm64" : null;
    if (!os || !arch) throw new Error(`No PDFium build for target ${triple}`);
    return `${os}-${arch}`;
  }
  const os = { win32: "win", darwin: "mac", linux: "linux" }[process.platform];
  const arch = { x64: "x64", arm64: "arm64" }[process.arch];
  if (!os || !arch) throw new Error(`No PDFium build for ${process.platform}-${process.arch}`);
  return `${os}-${arch}`;
}

function targetArg() {
  const i = process.argv.indexOf("--target");
  return i >= 0 ? process.argv[i + 1] : process.env.TAURI_ENV_TARGET_TRIPLE;
}

async function main() {
  const platform = platformFor(targetArg());
  const library = LIBRARY[platform.split("-")[0]];
  const stamp = `${VERSION} ${platform}\n`;
  const versionFile = join(outDir, "VERSION");

  if (existsSync(join(outDir, library.name)) && existsSync(versionFile) && readFileSync(versionFile, "utf8") === stamp) {
    console.log(`PDFium ${VERSION} (${platform}) is already in src-tauri/pdfium`);
    return;
  }

  const archive = `pdfium-${platform}.tgz`;
  const url = `https://github.com/bblanchon/pdfium-binaries/releases/download/${encodeURIComponent(VERSION)}/${archive}`;
  const bytes = await download(url);

  const digest = createHash("sha256").update(bytes).digest("hex");
  if (digest !== ARCHIVES[platform]) {
    throw new Error(`Checksum mismatch for ${archive}: expected ${ARCHIVES[platform]}, got ${digest}`);
  }

  const work = mkdtempSync(join(tmpdir(), "splitr-pdfium-"));
  try {
    writeFileSync(join(work, archive), bytes);
    // Relative paths and cwd keep this working with both bsdtar (Windows, macOS) and GNU tar.
    const tar = spawnSync("tar", ["-xzf", archive], { cwd: work, stdio: "inherit" });
    if (tar.status !== 0) throw new Error(`tar could not extract ${archive}`);

    rmSync(outDir, { recursive: true, force: true });
    mkdirSync(outDir, { recursive: true });
    copyFileSync(join(work, library.from), join(outDir, library.name));
    copyFileSync(join(work, "LICENSE"), join(outDir, "LICENSE"));
    writeFileSync(versionFile, stamp);
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
  console.log(`PDFium ${VERSION} (${platform}) saved to src-tauri/pdfium/${library.name}`);
}

main().catch((error) => {
  console.error(error.message);
  process.exit(1);
});
