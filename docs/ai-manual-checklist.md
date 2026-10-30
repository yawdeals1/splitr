# Manual check: real AI splits

The mocked tests (`cargo test`, see `src-tauri/src/smart_flow_tests.rs` and `ai_presets_split_and_export_with_a_mocked_provider` in `src-tauri/src/fixtures.rs`) check the Smart flow without a network: that only short excerpts go out, in overlapping chunks, and that answers are checked, retried once, merged and exported. They can't say whether a real model finds the right parts. This checklist covers that part (SRS 9.2, "Manual"), and measures the Phase 2 exit criterion: **AI splits are correct after at most 2 range edits on at least 80% of the sample files** (PRD 12, SRS 9.3).

## Before you start

- [ ] The test set from #12 is in `tests/fixtures/`. Use only the files whose right split is known, and write the expected parts down before running anything.
- [ ] An API key for the platform under test is saved in the app (AI section of the panel). It shows **Connected** after **Check**.
- [ ] **Use AI for Smart mode** is on, and you accepted the AI notice for that service.
- [ ] Note the app version (or commit), the AI service and the model.

## For each file

1. Open the file and pick Smart mode with the preset that fits it (for example Course material → By chapter for a textbook). Click **Split**.
2. Compare the parts on the review screen with the expected parts.
3. If they differ, fix them on the review screen and count the edits. One edit is one change to one boundary: moving a start or end, adding a split, joining two parts, or deleting a part. Renaming a title doesn't count.
4. Record the result in the table below.

A file **passes** when the parts are right after at most 2 edits. It **fails** when it needs 3 or more edits, or when Splitr shows an error instead of parts (note the message).

## While you're at it

- [ ] Nothing but the excerpts and the instruction left the computer. Read the AI notice; it says exactly what is sent (FR-61).
- [ ] The original PDF is unchanged afterwards (compare its size and date, or a checksum).
- [ ] Cancel works (once Smart mode has a Cancel button, #64): start a split on a large file and cancel it. No parts appear and no error about the key is shown.
- [ ] Turning AI off blocks Smart mode's AI presets with a clear message.

## Results

Service and model: ______________________  Version: ____________  Date: ____________

| File | Pages | Preset | Edits needed | Pass? | Notes |
| --- | --- | --- | --- | --- | --- |
| | | | | | |
| | | | | | |
| | | | | | |

**Score:** ____ of ____ files pass = ____%. Phase 2 exits at **80% or more**.

Run the checklist once for each service you want to call supported. The exit criterion needs one service at 80%; record the others too, so the user guide can say which work best.
