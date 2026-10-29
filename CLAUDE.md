# Splitr: project guide for coding agents

> CLAUDE.md and AGENTS.md contain the same text. When you change one, make the same change to the other in the same commit.

Splitr (working name) is Smart PDF Splitter: a local-first desktop app for Windows, macOS and Linux. It splits one PDF into several files by a condition the user picks, such as by chapter, by invoice or by student. Requirements live in `docs/PRD.md` (product) and `docs/SRS.md` (software). Requirement IDs such as FR-30 and NFR-05 come from the SRS; cite them in code comments and commits.

## Stack

- Tauri 2 shell, Rust backend in `src-tauri/`
- React 19 + TypeScript + Vite frontend in `src/`
- PDF engine: `pdfium-render` (render, text, outline) and `lopdf` (write parts)
- AI: Claude API over HTTPS from the Rust backend, with the user's own key

## Commands

| Task | Command |
| --- | --- |
| Install dependencies | `npm install` |
| Run the app | `npm run tauri dev` |
| Frontend type-check and build | `npm run build` |
| Rust tests | `cargo test --manifest-path src-tauri/Cargo.toml` |
| Rust lint | `cargo clippy --manifest-path src-tauri/Cargo.toml` |
| Release | push a tag `vX.Y.Z` (GitHub Actions builds installers) |

## Layout

- `src-tauri/src/commands.rs`: every Tauri command the frontend calls. Keep commands thin.
- `src-tauri/src/strategy/`: one module per split strategy behind the `Strategy` trait (NFR-14). Add a strategy as a new module; don't change the others.
- `src-tauri/src/ranges.rs`: range parsing and validation (FR-20, FR-35). Every proposed range list goes through `validate`.
- `src-tauri/src/naming.rs`: output file names (FR-51, FR-52).
- `src-tauri/src/pdf.rs`: the only place that touches PDF libraries.
- `presets/presets.json`: presets are data, not code (FR-72). The `bundled_presets_are_valid` test checks the file.
- `src/types.ts`: TypeScript mirrors of the Rust types. Update it whenever a serialized Rust type changes.

## Rules

### Security and privacy (never break these)

- Never commit secrets: API keys, tokens, passwords, signing keys, `.env` files or certificates. `.gitignore` blocks the common ones; check `git diff --staged` before every commit anyway.
- The user's API key is stored only in the OS credential store (the `keyring` crate). It must never be written to settings files or logs, sent to the frontend after it's saved, or bundled in the app (NFR-05).
- Signing keys and certificates live only in GitHub Actions encrypted secrets.
- Don't change environment variables, PATH or system settings from code, scripts or config in the repo. Don't commit machine-specific paths.
- Rule-based splitting makes no network calls (NFR-06). Only the AI strategy and the update check may use the network.
- The AI step sends short per-page excerpts, never the whole file, and only after the user has given consent (FR-61).
- Never modify the original PDF (FR-04). Write output to a temp file and rename it, so a failed export leaves no partial files (NFR-09).
- Delete temporary files (thumbnails) on exit (NFR-07).

### Code

- Rust: return `crate::error::Result`. Errors are user-facing messages, so make them specific ("This PDF is password protected"). No `unwrap()` outside tests and startup.
- Heavy work (rendering, extraction, AI calls) runs off the UI thread, in async commands or `spawn_blocking` (NFR-02).
- Every new piece of logic gets unit tests next to it (`#[cfg(test)] mod tests`).
- Match the surrounding style: small modules, short doc comments that cite requirement IDs.

### Workflow

- Work on a branch, open a PR, and merge only when CI is green.
- Run `cargo test` and `npm run build` before pushing.
- Commit messages: imperative summary line, then a body that names the requirement IDs touched.

## Build plan

Status key: [x] done, [ ] to do. Each phase ends when its exit criterion is met (PRD 12).

### Phase 0: Prototype (open, preview, custom split)
- [x] Tauri + React scaffold, CI, release workflow
- [x] Range parsing and validation, file naming, preset loading, with tests
- [ ] Bundle a PDFium library per platform (pdfium-binaries) as a Tauri resource, and load it in `pdf.rs`
- [ ] `open_pdf`: name, page count and size; password prompt; clear errors for corrupt files (FR-01 to FR-05)
- [ ] File picker and drag and drop (`@tauri-apps/plugin-dialog`)
- [ ] Lazy thumbnail grid with a larger page view; thumbnails cached in a per-session temp folder (FR-10, FR-11, NFR-03)
- [ ] `export_parts` with `lopdf`: write each range to a chosen folder, temp-then-rename, keep links and bookmarks where possible (FR-50, FR-54, NFR-09)
- **Exit:** a custom range split saves correctly on the developer's machine.

### Phase 1: Rule engine and review screen
- [x] Fixed, outline, pattern and blank page strategy logic, with tests
- [ ] Extract page text and the outline from PDFium into `DocumentText`
- [ ] `run_preset` command: run the preset's strategy, then its `fallback` on `NoOutline`/`NothingFound` (FR-30)
- [ ] Review screen: list of parts, editing boundaries, adding, removing, renaming and deleting parts, split marks on thumbnails (FR-12, FR-40 to FR-42)
- [ ] Naming template UI, optional ZIP, an "Open folder" button (FR-51 to FR-55)
- [ ] Settings file: theme, last folder, template, blank threshold (SRS 6.4)
- [ ] Build a test set of 10 or more varied PDFs in `tests/fixtures/`, using only public or generated files with no personal data
- **Exit:** 10 varied test PDFs split correctly with rules only.

### Phase 2: AI engine (Smart mode)
- [ ] API key settings stored with `keyring`, an AI on/off switch and a consent dialog (FR-60 to FR-62)
- [ ] Build excerpts (first and last lines of each page), in overlapping chunks for large files (FR-33, FR-34, NFR-04)
- [ ] Claude API client in Rust (`reqwest`, HTTPS), using the Appendix A prompt; the model is configurable in settings
- [ ] Strict JSON parsing, merging chunk results, `validate`, retrying once and then manual fixing (FR-35, FR-36)
- [ ] Save a custom prompt as a user preset (FR-71)
- [ ] Integration tests with a mocked AI provider
- **Exit:** 80% of AI test files are correct after at most 2 range edits.

### Phase 3: Packaging and updates
- [ ] Tauri updater plugin; updater signing key in GitHub secrets only (FR-64)
- [ ] Release workflow publishes the update manifest
- [ ] Light and dark themes with a manual toggle, plus a keyboard navigation pass (FR-63, NFR-11)
- [ ] Check that installers are under 25 MB (NFR-13)
- **Exit:** installers run on clean Windows, macOS and Linux machines.

### Phase 4: v1.0 release
- [ ] Fix bugs against the SRS 9.3 acceptance criteria
- [ ] Download page with SmartScreen and Gatekeeper instructions, and a user guide
- **Exit:** no open critical bugs.

### Phase 5: v1.1
- [ ] OCR for pages without a text layer (FR-80)
- [ ] Local AI model option (FR-81)
- **Exit:** scanned test PDFs split correctly.

## Open questions (from PRD 14)
- Final product name
- Personal tool or a product for sale (decides whether code signing and licensing are needed)
- Claude-only or multiple AI providers
- Maximum file size and page count, and languages for the interface and OCR
