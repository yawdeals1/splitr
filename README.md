# Splitr

Smart PDF Splitter: a local-first desktop app for Windows, macOS and Linux that splits one PDF into several files by a condition you pick, such as by chapter, by invoice or by student. Rule-based splits (bookmarks, text patterns, blank pages, fixed page counts) run offline. An optional AI step handles fuzzy conditions. Every split is previewed before any file is written.

"Splitr" is a working name.

- Product requirements: [docs/PRD.md](docs/PRD.md)
- Software requirements: [docs/SRS.md](docs/SRS.md)

## Stack

- [Tauri 2](https://tauri.app) shell with a Rust backend
- React, TypeScript and Vite frontend
- PDFium (`pdfium-render`) and `lopdf` for the PDF engine (Phase 0)
- Claude API for Smart (AI) mode, with the user's own key (Phase 2)

## Project layout

```
docs/                 PRD and SRS
presets/presets.json  Smart mode presets (data, not code)
src/                  React interface
src-tauri/src/
  commands.rs         Tauri commands called by the interface
  pdf.rs              PDF engine: open, render, extract, write
  ranges.rs           Range parsing and validation
  naming.rs           Output file name templates and sanitizing
  presets.rs          Preset loading
  strategy/           One module per split strategy (fixed, outline, pattern, blank, ai)
scripts/fetch-pdfium.mjs  Downloads the pinned PDFium build for a platform (checksum-verified)
.github/workflows/    CI (build and test) and Release (installers for 3 platforms)
```

## Development

Prerequisites: Node.js 20+, Rust (stable) and the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your OS.

```sh
npm install
npm run fetch-pdfium   # download the PDFium library into src-tauri/pdfium (not committed)
npm run tauri dev      # run the app
npm run build          # type-check and build the frontend
cd src-tauri && cargo test
```

## Releasing

Push a tag such as `v0.1.0`. The Release workflow builds `.msi`/`.exe`, `.dmg` (Apple Silicon and Intel), `.AppImage` and `.deb` installers and attaches them to a draft GitHub Release.

## Status

| Phase | Scope | State |
| --- | --- | --- |
| 0. Prototype | Tauri shell, open a PDF, thumbnails, custom range split | Scaffolded: range parsing done, PDF engine stubbed |
| 1. Rule engine | Fixed, outline, regex and blank page splits, review screen | Strategy logic and tests in place |
| 2. AI engine | Smart mode, presets, custom prompt | Presets defined, AI strategy stubbed |
| 3. Packaging | Installers for 3 platforms, auto-update | Release workflow in place |
