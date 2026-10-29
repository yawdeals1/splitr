# Work split

Who owns which open issue. Each issue already has its own branch on `origin`; check it out, do the work there, and open a PR that says `Closes #N`. The workflow rules in `CLAUDE.md` still apply: one issue per branch and PR, green CI, and the owner reviews and merges.

| Contributor | GitHub | Track |
| --- | --- | --- |
| Caleb Mensah (owner) | @yawdeals1 | PDF engine, core backend, AI core, security-sensitive and release work |
| Second contributor | @iamromelalvin7 | Frontend UI, settings, extra AI providers, tests, docs |

The split follows code areas, so the two tracks rarely touch the same files. Swap an issue by editing this file in a PR.

Status: "done, #PR" means merged; "in review, #PR" means a PR is open.

## Caleb (@yawdeals1)

| Issue | Title | Branch |
| --- | --- | --- |
| #2 | PDF engine: bundle and load PDFium (done, #37) | `phase-0/2-bundle-pdfium` |
| #3 | Open PDF: file info, password prompt and clear errors (done, #41) | `phase-0/3-open-pdf` |
| #6 | Export parts with lopdf (done, #49) | `phase-0/6-export-parts` |
| #7 | Extract page text and outline into DocumentText | `phase-1/7-extract-text` |
| #8 | run_preset command with fallback strategies | `phase-1/8-run-preset` |
| #12 | Test fixture set of 10+ PDFs | `phase-1/12-test-fixtures` |
| #13 | API key storage, AI toggle and consent notice (done, #53) | `phase-2/13-api-key-storage` |
| #14 | Page excerpts and overlapping chunks | `phase-2/14-excerpts-chunks` |
| #15 | Claude API client (done, #58) | `phase-2/15-claude-client` |
| #16 | AI response parsing, merging and validation (in review, #63) | `phase-2/16-ai-response-parsing` |
| #64 | Smart mode: run AI presets and show the result on the review screen | `phase-2/64-smart-ai-flow` |
| #31 | Connect a ChatGPT account (Sign in with ChatGPT) | `phase-2/31-chatgpt-signin` |
| #19 | Auto-update with signed update manifest | `phase-3/19-auto-update` |
| #21 | Installer size check and clean-machine install test | `phase-3/21-installer-size` |
| #24 | OCR for pages without a text layer | `phase-5/24-ocr` |
| #25 | Local AI model option | `phase-5/25-local-model` |

#19 needs the updater signing key in GitHub secrets, and #31 needs the owner to confirm OpenAI's developer terms, so both stay with the owner.

## @iamromelalvin7

| Issue | Title | Branch |
| --- | --- | --- |
| #4 | File picker and drag and drop (done, #38) | `phase-0/4-file-picker` |
| #5 | Lazy thumbnail grid and page viewer (done, #46) | `phase-0/5-thumbnail-grid` |
| #9 | Review and edit screen (done, #42) | `phase-1/9-review-screen` |
| #10 | Naming template, ZIP and open folder (done, #50) | `phase-1/10-naming-zip-open-folder` |
| #11 | Settings file (done, #39) | `phase-1/11-settings-file` |
| #17 | Save a custom prompt as a reusable preset (done, #40) | `phase-2/17-custom-prompt-preset` |
| #18 | Integration tests with a mocked AI provider | `phase-2/18-mock-ai-tests` |
| #27 | Bring your own key: multiple AI platforms with automatic key detection (in review, #44) | `phase-2/27-multi-key-detection` |
| #28 | OpenAI (GPT) provider (in review, #59) | `phase-2/28-openai-provider` |
| #29 | Google Gemini provider (in review, #60) | `phase-2/29-gemini-provider` |
| #30 | OpenAI-compatible platforms: OpenRouter, NVIDIA and others (in review, #61) | `phase-2/30-openai-compatible` |
| #32 | OpenCode Go provider (in review, #62) | `phase-2/32-opencode-go` |
| #20 | Theme toggle and keyboard navigation pass (done, #47) | `phase-3/20-theme-keyboard` |
| #22 | v1.0 acceptance pass | `phase-4/22-acceptance-pass` |
| #23 | Download page and user guide (done, #43) | `phase-4/23-download-page` |

## Other finished issues

Workflow issues and follow-ups from code reviews, outside the two tracks.

| Issue | Title | PR |
| --- | --- | --- |
| #1 | Repo workflow: PR template and issue-linked PRs | #26 |
| #33 | Update build plan for multiple AI platforms | #34 |
| #36 | Split open issues between the two contributors | #35 |
| #48 | Show split marks on thumbnails | #51 |
| #54 | Keyboard and theme fixes from the review of #47 | #55 |
| #56 | Pass each thumbnail its finished mark (review of #51) | #57 |

## Hand-offs between tracks

Some issues wait on other work. Start with work that has no blockers, and rebase on `main` once the blocker merges.

| Waiting issue | Needs first |
| --- | --- |
| #18 mocked AI tests (@iamromelalvin7) | #16 parsing (@yawdeals1, in review, #63) |
| #64 Smart mode AI flow (@yawdeals1) | #7 page text, #8 run_preset, #14 excerpts, #16 parsing (@yawdeals1) |
| #22 acceptance pass (@iamromelalvin7) | everything in phases 0 to 3 |

Unblocked starting points: Caleb on #7, #12 and #14, then #8 once #7 lands. @iamromelalvin7 on finishing the open provider PRs (#44, #59 to #62), then #18 once #63 merges. #18 can build its mock on `ai::Provider` and test `ai::response::split`, which takes a provider.

## Working on your branch

```sh
git fetch origin
git switch phase-0/4-file-picker   # your issue's branch
git rebase origin/main             # before you start and before you open the PR
```
