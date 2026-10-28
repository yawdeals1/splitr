# Smart PDF Splitter: Software Requirements Specification

Oct 28, 2026 · @Caleb Mensah

## 1. Introduction

This document specifies the software requirements for Smart PDF Splitter, a cross-platform desktop application that splits PDF files into several files according to a user-selected condition. It is written for the developer and for anyone who reviews or tests the product. Product goals and scope are in the companion Product Requirements Document (PRD).

**1.1 Scope.** The software opens a PDF, proposes split points using rule-based or AI-assisted methods, lets the user review and edit them, and writes the resulting PDFs to disk. It does not edit page content, merge, compress or convert files.

**1.2 Requirement keywords.** "Shall" marks a mandatory requirement. "Should" marks a recommended one. "May" marks an optional one. Each requirement has an ID (FR for functional, NFR for non-functional) so it can be traced to tests.

**1.3 Definitions.**

| Term | Meaning |
| --- | --- |
| Range | A start page and end page that become one output file |
| Outline | The bookmarks (table of contents) stored inside a PDF |
| Preset | A saved split recipe: category, name, strategy and prompt |
| Strategy | The method used to find split points: outline, pattern, blank page, fixed or AI |
| Excerpt | A short piece of page text (for example first and last lines) sent to the AI |
| OCR | Optical character recognition, used to read text from scanned pages |

## 2. Overall description

**2.1 Product perspective.** A standalone, installable desktop app. It needs no server of its own. The only external service it may call is an AI provider API, and only when the user runs an AI split.

**2.2 User classes.** One class in v1: a single end user working on their own computer. No accounts, roles or multi-user features.

**2.3 Operating environment.**

| Platform | Minimum version |
| --- | --- |
| Windows | Windows 10 (64-bit) or later, WebView2 runtime present or installed by the installer |
| macOS | macOS 11 or later, Apple Silicon and Intel |
| Linux | 64-bit distribution with WebKitGTK 4.1 (Ubuntu 22.04 or equivalent) |

**2.4 Constraints.**

- One shared codebase for all three platforms.
- Installers must be built on each target operating system.
- No store distribution, so no store sandbox requirements.
- The AI step requires an internet connection and a user-supplied API key.

**2.5 Assumptions and dependencies.**

- Input PDFs are not corrupt. Encrypted PDFs need the user to supply the password.
- PDFium is available for rendering and text extraction on all platforms.
- The AI provider returns valid JSON when instructed. The app validates it regardless.

## 3. System architecture

The interface talks to one backend. The backend tries rule-based strategies first and calls the AI provider only when a condition needs it.

&#91;embedded content: system architecture · 7 components\]

The backend is the hub: it loads presets, asks the PDF engine for pages, and hands page text to the strategy engines, which return ranges.

| Component | Responsibility |
| --- | --- |
| React interface | File drop, thumbnails, mode and preset selection, review and edit screen, settings |
| Backend commands | Open file, extract text, run a strategy, split and export, read settings, call the AI provider |
| PDF engine | Render thumbnails, extract page text, read the outline, write split PDFs |
| Strategy engines | One module per strategy (outline, pattern, blank, fixed, AI) behind a common interface |
| Preset library | JSON list of categories and presets, loaded at start |
| OS credential store | Secure storage for the user's API key |
| AI provider API | External service that turns excerpts and an instruction into page ranges |

**Smart split data flow**

1. The interface sends the file path, category, preset and any custom prompt to the backend.
2. The backend loads the preset and picks its strategy.
3. For outline, pattern and blank page strategies, the backend extracts what it needs from the PDF engine and computes ranges locally.
4. For the AI strategy, the backend extracts short excerpts per page, sends them in chunks with the instruction, and merges the returned ranges.
5. The backend validates the ranges and returns them to the interface.
6. The user reviews and edits them. On confirm, the backend writes the output files.

## 4. Functional requirements

**4.1 File handling**

| ID | Requirement |
| --- | --- |
| FR-01 | The system shall open a PDF by drag and drop or a file picker. |
| FR-02 | The system shall show the file name, page count and file size after opening. |
| FR-03 | The system shall prompt for a password when a PDF is encrypted, and report an error if it is wrong. |
| FR-04 | The system shall never modify the original file. |
| FR-05 | The system shall report a clear error for corrupt or unsupported files. |

**4.2 Page preview**

| ID | Requirement |
| --- | --- |
| FR-10 | The system shall render page thumbnails in a scrollable grid, loading them lazily for large files. |
| FR-11 | The system shall let the user open a single page at larger size. |
| FR-12 | The system shall mark proposed split boundaries on the thumbnail grid. |

**4.3 Range modes**

| ID | Requirement |
| --- | --- |
| FR-20 | Custom mode: the user shall enter page ranges such as 1-14, 15-30, 31. Invalid, overlapping or out-of-bounds ranges shall be rejected with a message. |
| FR-21 | Fixed mode: the user shall split every N pages or into N equal parts. |
| FR-22 | Smart mode: the user shall choose a document category and a split preset, or choose Custom prompt and type an instruction. |
| FR-23 | The preset list shall update when the category changes. |

**4.4 Split strategies**

| ID | Requirement |
| --- | --- |
| FR-30 | Outline strategy: the system shall read PDF bookmarks and propose a range per top-level bookmark. If no bookmarks exist, it shall fall back to the AI strategy or report that none were found. |
| FR-31 | Pattern strategy: the system shall extract page text and start a new range where a regex or keyword matches. |
| FR-32 | Blank page strategy: the system shall treat a page with no visible content (below a configurable threshold) as a separator and optionally drop it from the output. |
| FR-33 | AI strategy: the system shall build per-page excerpts, send them with the preset or custom prompt to the AI provider, and parse a JSON list of ranges. |
| FR-34 | For large files the AI strategy shall send pages in overlapping chunks and merge the results into one non-overlapping list. |
| FR-35 | The system shall validate every proposed range list: ranges in bounds, ascending, non-overlapping, and covering all pages unless the user chose to drop some. |
| FR-36 | If validation fails, the system shall retry the AI call once, then show the problem and let the user fix ranges manually. |

**4.5 Review and edit**

| ID | Requirement |
| --- | --- |
| FR-40 | The system shall show all proposed parts in a list with start page, end page, page count and title. |
| FR-41 | The user shall be able to change a boundary, add a split, remove a split, rename a part and delete a part. |
| FR-42 | In Smart mode, no file shall be written until the user confirms on the review screen. |

**4.6 Export**

| ID | Requirement |
| --- | --- |
| FR-50 | The system shall write each part as a separate PDF to a user-chosen folder. |
| FR-51 | The user shall set a file name template using tokens such as {index}, {title} and {original}. |
| FR-52 | The system shall sanitize file names for all three operating systems and avoid overwriting by adding a numeric suffix. |
| FR-53 | The system shall offer to bundle the outputs into a ZIP file. |
| FR-54 | The system shall preserve page content, links and bookmarks that fall inside each part where the PDF library allows. |
| FR-55 | After export, the system shall offer to open the output folder. |

**4.7 Settings and AI configuration**

| ID | Requirement |
| --- | --- |
| FR-60 | The user shall enter an AI provider API key, stored in the operating system credential store. |
| FR-61 | The system shall show a consent notice before the first AI use stating what text is sent. |
| FR-62 | The user shall be able to disable AI features. |
| FR-63 | The system shall support light and dark themes. |
| FR-64 | The system shall check for updates and let the user install them. |

**4.8 Presets**

| ID | Requirement |
| --- | --- |
| FR-70 | Presets shall load from a JSON file bundled with the app. |
| FR-71 | The user should be able to save their own custom prompt as a reusable preset. |
| FR-72 | Adding a preset shall need no code change. |

**4.9 Later releases (v1.1 and v1.2)**

| ID | Requirement |
| --- | --- |
| FR-80 | OCR: the system should run OCR on pages with no text layer before the Pattern and AI strategies. |
| FR-81 | Local model: the system should support a local AI model so Smart mode works offline. |
| FR-82 | Batch: the system may process several PDFs in one run. |
| FR-83 | Separator codes: the system may split at barcode or QR separator sheets. |

## 5. External interface requirements

**5.1 User interface.** A single main window with a left canvas for the file and thumbnails and a right panel for options, matching the reference layout: a range mode switch (Custom, Fixed, Smart), a document category dropdown, a split preset dropdown and a primary Split button. A second screen shows the review list. A settings dialog holds the API key, theme and AI consent.

**5.2 Hardware.** No special hardware. Recommended 8 GB of memory for PDFs over 500 pages.

**5.3 Software interfaces.**

| Interface | Purpose |
| --- | --- |
| PDF libraries (PDFium and lopdf) | Render, extract text, read outline, write split files |
| OCR engine (Tesseract or similar, v1.1) | Read text from scanned pages |
| OS credential store | Store the API key securely |
| File system | Read input, write output, temporary files |
| Updater service (GitHub Releases) | Check for and download new versions |

**5.4 Communications interfaces.**

- HTTPS requests to the AI provider API, only for AI splits.
- HTTPS requests to the release page for update checks.
- No other network traffic. Rule-based splitting works fully offline.

## 6. Data requirements

**6.1 Preset entry.** Presets are stored in a bundled JSON file. Each entry has the fields below.

```json
{
  "id": "academic-course-by-chapter",
  "category": "Academic and educational documents",
  "name": "Split course material by chapter",
  "strategy": "outline",
  "fallback": "ai",
  "prompt": "Split this course material so each chapter is one part. Use chapter titles as part titles.",
  "pattern": null
}
```

Allowed `strategy` values: `outline`, `pattern`, `blank`, `fixed`, `ai`. The `fallback` field is optional and names the strategy to use if the first finds nothing.

**6.2 AI request.** For each chunk the app sends the instruction and a list of pages with short excerpts.

```json
{
  "instruction": "Split course material by chapter",
  "pages": [
    { "page": 1, "head": "Preface", "tail": "..." },
    { "page": 2, "head": "Chapter 1 Computer Abstractions", "tail": "..." }
  ]
}
```

**6.3 AI response.** The app requires strict JSON in this shape.

```json
[
  { "start": 1, "end": 1, "title": "Preface" },
  { "start": 2, "end": 54, "title": "Chapter 1" }
]
```

**6.4 Stored settings.** Theme, last output folder, file name template, blank page threshold and AI consent status are stored in a local settings file. The API key is stored only in the operating system credential store.

## 7. Non-functional requirements

| ID | Area | Requirement |
| --- | --- | --- |
| NFR-01 | Performance | A rule-based split of a 300-page PDF shall finish analysis in under 15 seconds on a mid-range laptop. |
| NFR-02 | Performance | The interface shall stay responsive during analysis. Heavy work runs off the main thread or in the backend. |
| NFR-03 | Performance | Thumbnails shall load lazily. A 1,000-page PDF shall not load all pages into memory at once. |
| NFR-04 | Performance | The AI strategy shall send excerpts, not full text, so a 300-page file uses a small, bounded number of requests. |
| NFR-05 | Security | The API key shall be stored only in the OS credential store and never logged. |
| NFR-06 | Privacy | Without an AI split, the app shall make no network requests except update checks. |
| NFR-07 | Privacy | Temporary files shall be deleted on exit. |
| NFR-08 | Reliability | The app shall not crash on corrupt PDFs. It shall show an error and stay usable. |
| NFR-09 | Reliability | A failed export shall not leave partial files. |
| NFR-10 | Usability | A new user shall complete a basic split without reading documentation. |
| NFR-11 | Usability | All actions shall be reachable by keyboard. |
| NFR-12 | Portability | The same source code shall build for Windows, macOS and Linux. Platform-specific code shall be isolated. |
| NFR-13 | Size | The installer should be under 25 MB. |
| NFR-14 | Maintainability | Split strategies shall share one interface so a new strategy can be added without changing the others. |
| NFR-15 | Maintainability | Presets shall be data, not code. |
| NFR-16 | Compatibility | The app shall handle PDFs up to 2,000 pages and 500 MB. |

## 8. Build, packaging and deployment

1. The repository holds the frontend (React and Vite), the backend (Rust, using Tauri) and the preset JSON.
2. A GitHub Actions workflow runs a build matrix on Windows, macOS and Ubuntu runners for every tagged release.
3. Each runner produces its installer: .msi and .exe (Windows), .dmg (macOS), .AppImage and .deb (Linux).
4. The workflow attaches all installers to one GitHub Release and publishes the update manifest used by the auto-updater.
5. The app checks the manifest on start and offers to update when a newer version exists.
6. Installers are unsigned in v1. If signing is added later, the Windows certificate and Apple notarization credentials are stored as encrypted GitHub secrets, never in the repository.

| Platform | Dependency note |
| --- | --- |
| Windows | The installer installs the WebView2 runtime if missing |
| macOS | Build separate or universal binaries for Apple Silicon and Intel |
| Linux | The .deb declares WebKitGTK as a dependency. The AppImage bundles what it needs |

## 9. Testing and acceptance criteria

**9.1 Test set.** Keep a folder of at least 20 sample PDFs: a textbook with bookmarks, a textbook without bookmarks, an invoice batch, a contract bundle, scanned pages, a file with blank separators, an encrypted file, a corrupt file and a 1,000-page file.

**9.2 Test levels.**

| Level | What is tested |
| --- | --- |
| Unit | Range validation, regex matching, file name sanitizing, JSON response parsing, chunk merging |
| Integration | Open, split and export for each strategy, with a mocked AI provider |
| End to end | Full flow on Windows, macOS and Linux, including install and update |
| Manual | Real AI splits on the sample set, reviewed by a person |

**9.3 Acceptance criteria for v1.0.**

- Every Must requirement in sections 4.1 to 4.8 passes its tests on all three operating systems.
- Rule-based splits are correct on at least 90% of the sample files without manual edits.
- AI splits are correct after at most 2 range edits on at least 80% of the sample files.
- Exported parts open correctly in at least two PDF viewers.
- Original files are byte-for-byte unchanged after every test run.
- Installers install, launch and uninstall cleanly on a fresh machine for each platform.

## Appendix A. AI prompt template

The app builds the system prompt from the preset text and these fixed rules. Page numbers are 1-based and match the pages sent.

```text
You are given a list of pages from a PDF, each with a short excerpt.
Task: {preset prompt or custom prompt}
Rules:
- Return only a JSON array, no other text.
- Each item has: start (int), end (int), title (string).
- Ranges must not overlap and must use the page numbers provided.
- Ranges must be in ascending order.
- Cover every page unless the task says to skip some.
- If you are unsure where a part begins, choose the most likely page.
```

## Appendix B. Reference behavior

The reference product offers Custom, Fixed and Smart range modes. Smart mode asks for a document category and a split preset, or a custom prompt, then splits the file. This SRS reproduces that user-facing behavior with an original implementation and adds a mandatory review step before export.
