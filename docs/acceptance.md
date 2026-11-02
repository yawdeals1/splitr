# v1.0 acceptance pass

The checklist for #22: every SRS 9.3 criterion and every Must ("shall") requirement in SRS 4.1 to 4.8 and section 5 (NFR), with the evidence that it passes. "Auto" means a test in CI covers it; "Manual" means a person checks it on each operating system. An open issue next to a row is the work still needed before the row can be checked.

Status as of commit 5353d03 on `main` (2026-10-30), with phases 0 to 3 and #24, #25 merged. A row marked ⏳ still needs the check named; where an issue is named, it waits on that issue.

## SRS 9.3 criteria

| Criterion | How it's checked | Status |
| --- | --- | --- |
| Every Must requirement passes on Windows, macOS and Linux | The tables below, run on all three. CI runs tests on Linux only, so Windows and macOS are checked by hand with a release build | ⏳ |
| Rule-based splits correct on ≥ 90% of samples without edits | Auto: the `fixtures` integration tests split every rule fixture with the bundled presets and check each part. Manual: run each rule preset on the fixture set in the app; count files needing no edits | Auto ✅, manual ⏳ |
| AI splits correct on ≥ 80% after ≤ 2 range edits | `docs/ai-manual-checklist.md` on the fixture set, with a real provider | ⏳ manual |
| Exported parts open in at least two PDF viewers | Open every exported part from the runs above in two viewers (for example the browser and the OS viewer) | ⏳ |
| Originals byte-for-byte unchanged after every run | Auto: `export` tests and the `fixtures` integration tests compare the source before and after. Manual: checksum the fixtures before and after the runs | Auto ✅, manual ⏳ |
| Installers install, launch and uninstall cleanly on fresh machines | Size check in the release workflow (`scripts/check-installer-size.mjs`, #21); clean VM per platform with the first release | Size ✅, clean install ⏳ #83 |

## 4.1 File handling

| Req | Evidence | Status |
| --- | --- | --- |
| FR-01 open by drag and drop or file picker | `file.test.ts` (drop rules); manual: drop, picker, Ctrl+O | Auto ✅, manual ⏳ |
| FR-02 show name, page count, size | `pdf::tests::opens_a_normal_pdf`, `file.test.ts` (`formatSize`) | ✅ |
| FR-03 password prompt, wrong-password error | `pdf::tests` (encrypted files), `export` tests with passwords | ✅ |
| FR-04 never modify the original | `export::tests::saves_each_part_and_leaves_the_original_unchanged`, ZIP test, `fixtures` integration tests (`export_and_check`, `ai_presets_split_and_export_with_a_mocked_provider`) | ✅ |
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
| FR-21 every N pages or N equal parts | `strategy::fixed` tests, `preset_run::tests::fixed_presets_split_every_n_pages`; manual: equal parts in the panel | Auto ✅, manual ⏳ |
| FR-22 category, preset or custom prompt | Manual | ⏳ |
| FR-23 preset list follows the category | Manual | ⏳ |

## 4.4 Split strategies

| Req | Evidence | Status |
| --- | --- | --- |
| FR-30 outline, with fallback | `strategy::outline` tests, `preset_run` fallback tests, `fixtures` textbook and single-root tests | ✅ |
| FR-31 pattern | `strategy::pattern` tests; `fixtures` invoice, statement, employee, claim and bill of lading tests | ✅ |
| FR-32 blank pages, configurable threshold | `strategy::blank` tests, `preset_run::tests::the_blank_threshold_is_capped`, `blank.test.ts`; manual: the sensitivity setting | Auto ✅, manual ⏳ |
| FR-32 optionally drop blank pages from the output | `preset_run::tests::blank_pages_use_the_threshold_and_can_be_dropped`, `fixtures::tests::letters_split_at_blank_pages_which_can_be_dropped`, `smart.test.ts` (drop option); manual: the drop toggle | Auto ✅, manual ⏳ |
| FR-33 excerpts sent with the prompt, JSON parsed | `ai::excerpts` tests, `ai::response` and provider tests | ✅ |
| FR-34 overlapping chunks merged | `ai::response` merge tests, `smart_flow_tests` | ✅ |
| FR-35 every range list validated | `ranges::validate` tests, `ai::response` tests | ✅ |
| FR-36 one retry, then manual fixing | `ai::response` tests, `smart_flow_tests`; manual: the reply problem on the review screen | Auto ✅, manual ⏳ |

## 4.5 Review and edit

| Req | Evidence | Status |
| --- | --- | --- |
| FR-40 list of parts with pages and titles | `review.test.ts`; manual | ✅ |
| FR-41 change, add, remove, rename, delete | `review.test.ts` | ✅ |
| FR-42 nothing written before confirming | Export is only reachable from the review screen, in rule and Smart mode; manual | Auto ✅, manual ⏳ |

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
| FR-60 key in the OS credential store, one entry per platform | `keystore` tests, `key_detect` tests; manual on each OS's store | Auto ✅, manual ⏳ |
| FR-61 consent notice before first AI use | Manual | ⏳ |
| FR-62 AI can be turned off | `ai.test.ts` (`aiBlock`); manual | ✅ |
| FR-63 light and dark themes | `theme.test.ts`; manual | ✅ |
| FR-64 check for and install updates | `update.test.ts`; signed manifest from the release workflow (#19); manual: an installed copy updates to the next release | Auto ✅, manual ⏳ #83 |

## 4.8 Presets

| Req | Evidence | Status |
| --- | --- | --- |
| FR-70 presets from a bundled JSON file | `presets::tests::bundled_presets_are_valid` | ✅ |
| FR-72 adding a preset needs no code change | Presets are data (`presets/presets.json`) | ✅ |

## Non-functional requirements

| Req | Evidence | Status |
| --- | --- | --- |
| NFR-01 rule split of 300 pages analysed in < 15 s | `fixtures::tests::a_1000_page_file_splits_and_exports_in_time`; manual: time it on a mid-range laptop | Auto ✅, manual ⏳ |
| NFR-02 interface stays responsive | Heavy commands run in `spawn_blocking` or async; manual | ✅ (manual ⏳) |
| NFR-03 lazy thumbnails, bounded memory | Render queue and cache tests; manual with 1,000 pages | ⏳ manual |
| NFR-04 bounded AI requests for 300 pages | `ai::excerpts` tests (limits and overlapping chunks), `smart_flow_tests` | ✅ |
| NFR-05 key only in the credential store, never logged | `keystore` tests; provider tests check errors never contain the key | ✅ |
| NFR-06 no network without an AI split | Only AI commands and the update check create HTTP clients; review | ✅ |
| NFR-07 temporary files deleted on exit | `viewer` session cleanup on exit and at start | ✅ |
| NFR-08 no crash on corrupt PDFs | `pdf` tests; manual with the corrupt fixture | Auto ✅, manual ⏳ |
| NFR-09 failed export leaves no partial files | `export::tests::a_failed_export_leaves_no_files`, rename and ZIP failure tests | ✅ |
| NFR-10 a new user splits without the docs | Ask someone who hasn't used Splitr to split a PDF, and note where they get stuck | ⏳ manual |
| NFR-11 everything reachable by keyboard | Keyboard pass (#20, #54); manual | ✅ (manual ⏳) |
| NFR-12 builds for all three OSes | Release workflow builds Windows, macOS and Linux | ⏳ first release (#83) |
| NFR-12 platform-specific code isolated | Review: OS differences go through crates (`keyring`, `tauri-plugin-opener`, `open`, PDFium's library name in `pdf.rs`); the only `cfg(unix)` block is the session folder permissions in `viewer.rs` | ✅ |
| NFR-14 strategies share one interface | `Strategy` trait | ✅ |
| NFR-15 presets are data | `presets/presets.json` | ✅ |
| NFR-16 up to 2,000 pages and 500 MB | No fixture this large: the generated file has 1,000 pages and is small. Build a 2,000-page file of about 500 MB (for example scanned pages) and open, preview, split and export it | ⏳ manual, needs a large file |

## Open critical bugs

None known. Bugs found in the pass get an issue each, listed here, and are fixed before #22 closes.
