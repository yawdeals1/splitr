# v1.0 acceptance pass

The checklist for #22: every SRS 9.3 criterion and every Must ("shall") requirement in SRS 4.1 to 4.8 and section 5 (NFR), with the evidence that it passes. "Auto" means a test in CI covers it; "Manual" means a person checks it on each operating system. An open issue next to a row is the work still needed before the row can be checked.

Status as of 2026-10-30. Rows marked ⏳ wait on the issue named.

## SRS 9.3 criteria

| Criterion | How it's checked | Status |
| --- | --- | --- |
| Every Must requirement passes on Windows, macOS and Linux | The tables below, run on all three. CI runs tests on Linux only, so Windows and macOS are checked by hand with a release build | ⏳ |
| Rule-based splits correct on ≥ 90% of samples without edits | Run each rule preset on the #12 fixture set; count files needing no edits | ⏳ #12, #8 |
| AI splits correct on ≥ 80% after ≤ 2 range edits | `docs/ai-manual-checklist.md` on the fixture set | ⏳ #12, #64 |
| Exported parts open in at least two PDF viewers | Open every exported part from the runs above in two viewers (for example the browser and the OS viewer) | ⏳ |
| Originals byte-for-byte unchanged after every run | Auto: export tests compare the source before and after. Manual: checksum the fixtures before and after the runs | Auto ✅, manual ⏳ |
| Installers install, launch and uninstall cleanly on fresh machines | Clean VM per platform | ⏳ #21 |

## 4.1 File handling

| Req | Evidence | Status |
| --- | --- | --- |
| FR-01 open by drag and drop or file picker | `file.test.ts` (drop rules); manual: drop, picker, Ctrl+O | Auto ✅, manual ⏳ |
| FR-02 show name, page count, size | `pdf::tests::opens_a_normal_pdf`, `file.test.ts` (`formatSize`) | ✅ |
| FR-03 password prompt, wrong-password error | `pdf::tests` (encrypted files), `export` tests with passwords | ✅ |
| FR-04 never modify the original | `export::tests::saves_each_part_and_leaves_the_original_unchanged`, ZIP test, `smart_flow_tests` | ✅ |
| FR-05 clear error for corrupt or unsupported files | `pdf::tests::corrupt_and_non_pdf_files_give_clear_errors` | ✅ |

## 4.2 Page preview

| Req | Evidence | Status |
| --- | --- | --- |
| FR-10 lazy thumbnail grid | `viewer` tests, `thumbnails.test.ts`; manual: scroll a 1,000-page file | Auto ✅, manual ⏳ |
| FR-11 larger page view | Manual | ⏳ |
| FR-12 split marks on thumbnails | `marks.test.ts` | ✅ |

## 4.3 Range modes

| Req | Evidence | Status |
| --- | --- | --- |
| FR-20 Custom ranges, with errors for bad input | `ranges` tests | ✅ |
| FR-21 every N pages or N equal parts | `strategy::fixed` tests; equal parts in the panel | ⏳ #70 (PR #71) |
| FR-22 category, preset or custom prompt | Manual | ⏳ #64 for the AI run itself |
| FR-23 preset list follows the category | Manual | ⏳ |

## 4.4 Split strategies

| Req | Evidence | Status |
| --- | --- | --- |
| FR-30 outline, with fallback | `strategy::outline` tests; running it on a real PDF | ⏳ #7 (PR #66), #8 |
| FR-31 pattern | `strategy::pattern` tests; page text from the PDF | ⏳ #7, #8 |
| FR-32 blank pages, configurable threshold | `strategy::blank` tests; threshold setting | ⏳ #68 (PR #69), #8 |
| FR-33 excerpts sent with the prompt, JSON parsed | `ai::response` and provider tests; excerpt building | ⏳ #14 |
| FR-34 overlapping chunks merged | `ai::response` merge tests, `smart_flow_tests` | ✅ |
| FR-35 every range list validated | `ranges::validate` tests, `ai::response` tests | ✅ |
| FR-36 one retry, then manual fixing | `ai::response` tests, `smart_flow_tests` | ✅ (UI ⏳ #64) |

## 4.5 Review and edit

| Req | Evidence | Status |
| --- | --- | --- |
| FR-40 list of parts with pages and titles | `review.test.ts`; manual | ✅ |
| FR-41 change, add, remove, rename, delete | `review.test.ts` | ✅ |
| FR-42 nothing written before confirming | Export is only reachable from the review screen; manual | ✅ (Smart ⏳ #64) |

## 4.6 Export

| Req | Evidence | Status |
| --- | --- | --- |
| FR-50 one PDF per part in a chosen folder | `export` tests | ✅ |
| FR-51 name template | `naming` tests, `naming.test.ts` | ✅ |
| FR-52 safe names, numeric suffix | `naming` tests, `export::tests::names_avoid_existing_files_and_each_other` | ✅ |
| FR-53 ZIP | `export::tests::zips_the_parts_into_one_file` | ✅ |
| FR-54 links and bookmarks kept where possible | `pdf` tests on bookmarks and links in parts | ✅ |
| FR-55 open the output folder | Manual on each OS | ⏳ |

## 4.7 Settings and AI configuration

| Req | Evidence | Status |
| --- | --- | --- |
| FR-60 key in the OS credential store | `keystore` tests; manual on each OS's store | Auto ✅, manual ⏳ (multi-key: PR #44) |
| FR-61 consent notice before first AI use | Manual | ⏳ |
| FR-62 AI can be turned off | `ai.test.ts` (`aiBlock`); manual | ✅ |
| FR-63 light and dark themes | `theme.test.ts`; manual | ✅ |
| FR-64 check for and install updates | | ⏳ #19 |

## 4.8 Presets

| Req | Evidence | Status |
| --- | --- | --- |
| FR-70 presets from a bundled JSON file | `presets::tests::bundled_presets_are_valid` | ✅ |
| FR-72 adding a preset needs no code change | Presets are data (`presets/presets.json`) | ✅ |

## Non-functional requirements

| Req | Evidence | Status |
| --- | --- | --- |
| NFR-01 rule split of 300 pages analysed in < 15 s | Time it on a mid-range laptop | ⏳ #8 |
| NFR-02 interface stays responsive | Heavy commands run in `spawn_blocking` or async; manual | ✅ (manual ⏳) |
| NFR-03 lazy thumbnails, bounded memory | Render queue and cache tests; manual with 1,000 pages | ⏳ manual |
| NFR-04 bounded AI requests for 300 pages | Excerpt chunking | ⏳ #14 |
| NFR-05 key only in the credential store, never logged | `keystore` tests; provider tests check errors never contain the key | ✅ |
| NFR-06 no network without an AI split | Only AI commands and the update check create HTTP clients; review | ✅ (update check ⏳ #19) |
| NFR-07 temporary files deleted on exit | `viewer` session cleanup on exit and at start | ✅ |
| NFR-08 no crash on corrupt PDFs | `pdf` tests; manual with the corrupt fixture | Auto ✅, manual ⏳ |
| NFR-09 failed export leaves no partial files | `export::tests::a_failed_export_leaves_no_files`, rename and ZIP failure tests | ✅ |
| NFR-10 a new user splits without the docs | Ask someone who hasn't used Splitr to split a PDF, and note where they get stuck | ⏳ manual |
| NFR-11 everything reachable by keyboard | Keyboard pass (#20, #54); manual | ✅ (manual ⏳) |
| NFR-12 builds for all three OSes | Release workflow builds Windows, macOS and Linux | ⏳ first release |
| NFR-14 strategies share one interface | `Strategy` trait | ✅ |
| NFR-15 presets are data | `presets/presets.json` | ✅ |
| NFR-16 up to 2,000 pages and 500 MB | Manual with the large fixture | ⏳ #12 |

## Open critical bugs

None known. Bugs found in the pass get an issue each, listed here, and are fixed before #22 closes.
