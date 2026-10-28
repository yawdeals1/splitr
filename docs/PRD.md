# Smart PDF Splitter: Product Requirements Document

Oct 28, 2026 · @Caleb Mensah

## 1. Overview

Smart PDF Splitter is a desktop app for Windows, macOS and Linux that splits one PDF into several files based on a condition the user picks, such as "by chapter", "by invoice" or "by student". It combines fast rule-based methods (bookmarks, text patterns, blank pages, fixed page counts) with an optional AI step for fuzzy conditions, and always shows a preview of the proposed splits before any file is written.

- **Working name:** to be decided
- **Document status:** Draft v0.1
- **Companion document:** Software Requirements Specification (SRS)
- **Product type:** Installable desktop app, distributed as direct downloads (no app stores)

## 2. Problem and opportunity

Online PDF tools can split a file by page ranges, but splitting by meaning (every chapter, every invoice, every exam) is slow to do by hand and most tools that automate it upload the user's files to a server. That is a poor fit for contracts, medical records, HR files and student work.

- Splitting long documents manually means scrolling, noting page numbers and typing ranges, which is error-prone.
- Web tools require an upload, add file-size limits and need an internet connection.
- Books, scanned batches and mixed-document PDFs rarely split cleanly with fixed rules alone.

The opportunity is a local-first app that does the cheap, deterministic splits offline and uses AI only when a condition cannot be expressed as a rule.

## 3. Goals, non-goals and success metrics

**Goals**

1. Split a PDF by a chosen condition in under one minute for a typical 300-page document, including preview and review.
2. Run on Windows, macOS and Linux from one codebase.
3. Keep all files on the user's machine by default.
4. Offer a library of ready-made presets grouped by document category, plus a custom prompt.
5. Let the user correct any proposed split before exporting.

**Non-goals for v1**

- Editing PDF content (text, images, annotations).
- Merging, compressing or converting PDFs.
- Cloud accounts, sync or team features.
- Mobile apps and app store distribution.

**Success metrics**

| Metric | Target |
| --- | --- |
| Rule-based splits (outline, regex, fixed, blank page) that need no manual correction | 90% or more of test files |
| AI splits that are correct after at most 2 range edits | 80% or more of test files |
| Time from opening a file to exported results (300 pages, rule-based) | Under 30 seconds |
| Crash-free sessions | 99% or more |
| Installer size (Tauri build) | Under 25 MB |

## 4. Target users

| Persona | Typical files | What they need |
| --- | --- | --- |
| Student or lecturer | Textbooks, lecture packs, scanned homework, exam scripts | Split by chapter, topic, student or question |
| Office or admin staff | Invoices, bank statements, HR forms, government forms | Split a batch into one file per document or per person |
| Legal, medical or finance professional | Contracts, patient records, reports | Split sensitive files without uploading them anywhere |
| Content or marketing team | Brand books, creative packs, reports | Pull out sections to share separately |

The first user is the product owner, who needs it for study material and client documents. Version 1 is built for a single user on their own machine.

## 5. Use cases and user stories

1. As a student, I want to split a textbook PDF into one file per chapter so I can study and share chapters separately.
2. As an admin, I want to split a 200-page invoice batch into one PDF per invoice so I can file each one.
3. As a lecturer, I want to split scanned exam scripts by student ID so I can return each script individually.
4. As a professional, I want to split a contract bundle without uploading it, so confidential files stay on my computer.
5. As any user, I want to see the proposed split points on page thumbnails and adjust them before saving.
6. As any user, I want to write my own instruction (for example, "split whenever a new client name appears") when no preset fits.
7. As any user, I want to name output files from the detected titles, such as "Chapter 3 - Memory Hierarchy.pdf".

## 6. Features and scope

| Feature | Description | Priority | Release |
| --- | --- | --- | --- |
| Open PDF | Drag and drop or file picker, single file | Must | v1 |
| Page thumbnails | Scrollable grid of page previews | Must | v1 |
| Custom ranges | User types ranges such as 1-14, 15-30 | Must | v1 |
| Fixed split | Every N pages, or into N equal parts | Must | v1 |
| Bookmark (outline) split | Split at top-level bookmarks | Must | v1 |
| Pattern split | Split where page text matches a regex or keyword | Must | v1 |
| Blank page split | Split at blank separator pages | Should | v1 |
| Smart (AI) split | Presets and custom prompt, results shown as ranges | Must | v1 |
| Preview and edit | Drag, add, delete and rename proposed ranges | Must | v1 |
| Output naming | Template using detected title and index | Should | v1 |
| Export | Save all parts to a chosen folder, optional ZIP | Must | v1 |
| OCR for scanned PDFs | Extract text from image-only pages | Should | v1.1 |
| Local AI model option | Run the AI step offline through a local model | Should | v1.1 |
| Batch mode | Process several PDFs in one run | Could | v1.2 |
| Barcode or QR separator sheets | Split scanned batches by separator codes | Could | v1.2 |
| Auto-update | Check for and install new versions | Must | v1 |

## 7. Split modes, categories and presets

The app offers three range modes, matching the reference product: **Custom** (manual ranges), **Fixed** (every N pages) and **Smart** (category plus preset, or a custom prompt). Each Smart preset is a small configuration entry that names its strategy: read the PDF outline, match a text pattern, or ask the AI.

Categories to ship in v1:

| Category | Example presets | Default strategy |
| --- | --- | --- |
| Invoices and billing documents | Split by invoice number, split by vendor | Pattern, then AI |
| Contracts and legal documents | Split by agreement, separate exhibits and schedules | AI |
| Bank statements and financial reports | Split by account, split by month | Pattern, then AI |
| Academic and educational documents | Course material by chapter, exams by student, notes by topic, answer sheets from question sheets | Outline, then AI |
| Medical and healthcare records | Split by patient, split by visit date | AI |
| HR and employee documents | Split by employee | Pattern, then AI |
| Insurance documents | Split by policy or claim | Pattern, then AI |
| Shipping, logistics and customs documents | Split by shipment or bill of lading | Pattern, then AI |
| Government and administrative forms | Split by form type or applicant | AI |
| Scanned document batches | Split at blank pages or separator sheets | Blank page, then AI |
| Marketing and creative documents | Split by section or campaign | AI |
| Books and general reading documents | Split by chapter or part | Outline, then AI |
| Custom prompt | User writes the instruction | AI |

Presets live in a JSON file so new ones can be added without changing app code. Each preset stores its category, name, strategy, prompt text and optional regex.

## 8. User flow and UX requirements

**Main flow**

1. The user drops a PDF onto the window or picks it with a file dialog. A thumbnail appears.
2. The user chooses a range mode: Custom, Fixed or Smart.
3. In Smart mode, the user picks a document category and a split preset, or chooses Custom prompt and types an instruction.
4. The user clicks Split. A progress indicator shows extraction and analysis.
5. A review screen lists the proposed parts with page ranges, titles and thumbnails.
6. The user adjusts ranges, renames parts, or deletes parts they do not want.
7. The user chooses an output folder and naming template, then exports.
8. The app shows the result with a button to open the output folder.

**UX requirements**

- The review screen is mandatory for Smart mode. No file is written before the user confirms.
- Dropdowns for category and preset follow the reference layout: the preset list changes with the selected category.
- Long operations show progress and can be cancelled.
- The app never modifies the original PDF.
- Errors are specific, for example "This PDF is password protected" or "No text found on pages 1 to 40, try enabling OCR".
- The interface supports keyboard navigation and a light and a dark theme.

## 9. Technical approach summary

The full design is in the SRS. In short:

- **Shell and UI:** Tauri with a React and Vite frontend and a Rust backend.
- **PDF handling:** page rendering and text extraction through PDFium (pdfium-render), splitting through lopdf.
- **Two split engines:** a rule engine (outline, regex, blank pages, fixed counts) that runs offline, and an AI engine that sends short per-page excerpts to a language model and receives page ranges as JSON.
- **AI provider:** the Claude API using the user's own API key in v1, with a local model option in v1.1.
- **Presets:** a JSON library, one entry per preset.
- **Build and release:** GitHub Actions builds Windows, macOS and Linux installers and publishes them to GitHub Releases.

## 10. Privacy and security

- PDFs are processed on the user's machine. Rule-based splitting makes no network calls.
- In AI mode, only short text excerpts and page numbers are sent to the AI provider, never the whole file. The app states this clearly before the first AI use and asks for consent.
- The API key is entered by the user and stored in the operating system's secure credential store (Windows Credential Manager, macOS Keychain, Linux Secret Service). No key is bundled in the app.
- The user can disable AI features entirely.
- The app does not collect analytics in v1. If crash reporting is added later, it must be opt-in and must never include document content.
- Temporary files, such as rendered thumbnails, are stored in a per-session folder and deleted when the app closes.

## 11. Distribution and updates

The app is not published to any app store. Users download an installer for their operating system from GitHub Releases or a simple download page.

| Platform | Installer formats |
| --- | --- |
| Windows | .msi and .exe (NSIS) |
| macOS | .dmg, with separate Apple Silicon and Intel builds if needed |
| Linux | .AppImage and .deb |

- Each installer is built on its own operating system through a GitHub Actions matrix.
- Installers are unsigned in v1. The download page tells users how to get past the Windows SmartScreen warning and the macOS Gatekeeper prompt. Code signing is reconsidered if the app is shared widely or sold.
- The app checks the release page for new versions and offers one-click updates.

## 12. Release plan

Phases are ordered by dependency. Each phase ends when its exit criterion is met. No calendar dates are set yet.

| Phase | Scope | Exit criterion |
| --- | --- | --- |
| 0. Prototype | Tauri shell, open a PDF, thumbnails, custom range split | Splits and saves correctly on the developer's machine |
| 1. Rule engine | Fixed, outline, regex and blank page splits, preview and edit screen | 10 varied test PDFs split correctly with rules only |
| 2. AI engine | Smart mode, preset library, custom prompt, chunking, JSON validation | 80% of AI test files correct after at most 2 edits |
| 3. Packaging | GitHub Actions builds for three platforms, auto-update | Installers run on clean Windows, macOS and Linux machines |
| 4. v1.0 release | Bug fixes, download page, user guide | No open critical bugs |
| 5. v1.1 | OCR and local AI model option | Scanned test PDFs split correctly |

## 13. Risks and mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| AI proposes wrong split points | Wrong output files | Mandatory preview and edit screen, strict JSON validation, rule engine first |
| Very large PDFs exceed AI context or cost | Failures or high cost | Send short per-page excerpts in overlapping chunks and merge results |
| Scanned PDFs have no text layer | Smart mode finds nothing | OCR in v1.1, clear error message until then |
| Webview differences between operating systems (Tauri) | Layout or rendering bugs | Test on all three systems early and avoid webview-specific features |
| Unsigned installers trigger OS warnings | Users distrust or abandon install | Download page with instructions, plan for signing later |
| Exposed API key | Cost and abuse | User-supplied key stored in the OS credential store, never bundled |
| Corrupt or encrypted PDFs | Crashes or confusing errors | Validate on open, specific error messages, password prompt |

## 14. Open questions

- [ ] Final product name.
- [ ] Do the Rust PDF libraries (pdfium-render, lopdf) cover every split strategy reliably? Confirm in the Phase 0 prototype.
- [ ] Will the app stay a personal tool, or be sold? This decides whether code signing and licensing are needed.
- [ ] Should the AI provider be limited to the Claude API, or support other providers and local models from the start?
- [ ] Maximum supported file size and page count.
- [ ] Languages for the interface and for OCR.
