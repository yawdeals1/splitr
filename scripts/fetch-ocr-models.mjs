#!/usr/bin/env node
// Downloads the OCR models into src-tauri/ocr/ (FR-80, SRS 5.3). Tauri bundles that folder as a
// resource and ocr.rs loads the models from it the first time a scanned page is read. The models
// are the same on every platform. They come from the ocrs project and are never committed.
//
// Usage: node scripts/fetch-ocr-models.mjs
// It skips the download when the right models are already in place.

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { download } from "./download.mjs";

const BASE = "https://ocrs-models.s3-accelerate.amazonaws.com";

// SHA-256 of each model. Change these together with the ocrs version in src-tauri/Cargo.toml.
const MODELS = {
  "text-detection.onnx": "a917b23dbd9524b465df7e922641b3ff2981623df4ded5a0234004ef2fee7cfe",
  "text-recognition.onnx": "86c145c2edb96c8caed5b1ebb8f44d706408922211451c309c625157dd6061c5",
};

// The models are CC BY-SA 4.0, which asks for credit wherever they are shared.
const NOTICE = `OCR models (text-detection.onnx, text-recognition.onnx)

From the ocrs project by Robert Knight: https://github.com/robertknight/ocrs-models
Licensed under Creative Commons Attribution-ShareAlike 4.0 International (CC BY-SA 4.0):
https://creativecommons.org/licenses/by-sa/4.0/
Trained on the HierText dataset (CC BY-SA 4.0). Splitr uses the models unchanged.
`;

const outDir = join(dirname(fileURLToPath(import.meta.url)), "..", "src-tauri", "ocr");

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

async function main() {
  mkdirSync(outDir, { recursive: true });
  for (const [name, expected] of Object.entries(MODELS)) {
    const file = join(outDir, name);
    if (existsSync(file) && sha256(readFileSync(file)) === expected) {
      console.log(`${name} is already in src-tauri/ocr`);
      continue;
    }
    const bytes = await download(`${BASE}/${name}`);
    const digest = sha256(bytes);
    if (digest !== expected) throw new Error(`Checksum mismatch for ${name}: expected ${expected}, got ${digest}`);
    writeFileSync(file, bytes);
    console.log(`${name} saved to src-tauri/ocr`);
  }
  writeFileSync(join(outDir, "NOTICE"), NOTICE);
}

main().catch((error) => {
  console.error(error.message);
  process.exit(1);
});
