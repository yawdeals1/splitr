# Work split

Who owns which open issue. Each issue already has its own branch on `origin`; check it out, do the work there, and open a PR that says `Closes #N`. The workflow rules in `CLAUDE.md` still apply: one issue per branch and PR, green CI, and the owner reviews and merges.

| Contributor | GitHub | Track |
| --- | --- | --- |
| Caleb Mensah (owner) | @yawdeals1 | PDF engine, core backend, AI core, security-sensitive and release work |
| Second contributor | @iamromelalvin7 | Frontend UI, settings, extra AI providers, tests, docs |

The split follows code areas, so the two tracks rarely touch the same files. Swap an issue by editing this file in a PR.

Status: ✅ means merged; "In review" means a PR is open.

## Caleb (@yawdeals1)

| Status | Issue | Title | Branch |
| --- | --- | --- | --- |
| ✅ Done (#37) | #2 | PDF engine: bundle and load PDFium | `phase-0/2-bundle-pdfium` |
| ✅ Done (#41) | #3 | Open PDF: file info, password prompt and clear errors | `phase-0/3-open-pdf` |
| ✅ Done (#49) | #6 | Export parts with lopdf | `phase-0/6-export-parts` |
|  | #7 | Extract page text and outline into DocumentText | `phase-1/7-extract-text` |
|  | #8 | run_preset command with fallback strategies | `phase-1/8-run-preset` |
|  | #12 | Test fixture set of 10+ PDFs | `phase-1/12-test-fixtures` |
| ✅ Done (#53) | #13 | API key storage, AI toggle and consent notice | `phase-2/13-api-key-storage` |
|  | #14 | Page excerpts and overlapping chunks | `phase-2/14-excerpts-chunks` |
| ✅ Done (#58) | #15 | Claude API client | `phase-2/15-claude-client` |
| ✅ Done (#63) | #16 | AI response parsing, merging and validation | `phase-2/16-ai-response-parsing` |
|  | #64 | Smart mode: run AI presets and show the result on the review screen | `phase-2/64-smart-ai-flow` |
|  | #31 | Connect a ChatGPT account (Sign in with ChatGPT) | `phase-2/31-chatgpt-signin` |
|  | #19 | Auto-update with signed update manifest | `phase-3/19-auto-update` |
|  | #21 | Installer size check and clean-machine install test | `phase-3/21-installer-size` |
|  | #24 | OCR for pages without a text layer | `phase-5/24-ocr` |
|  | #25 | Local AI model option | `phase-5/25-local-model` |

#19 needs the updater signing key in GitHub secrets, and #31 needs the owner to confirm OpenAI's developer terms, so both stay with the owner.

## @iamromelalvin7

| Status | Issue | Title | Branch |
| --- | --- | --- | --- |
| ✅ Done (#38) | #4 | File picker and drag and drop | `phase-0/4-file-picker` |
| ✅ Done (#46) | #5 | Lazy thumbnail grid and page viewer | `phase-0/5-thumbnail-grid` |
| ✅ Done (#42) | #9 | Review and edit screen | `phase-1/9-review-screen` |
| ✅ Done (#50) | #10 | Naming template, ZIP and open folder | `phase-1/10-naming-zip-open-folder` |
| ✅ Done (#39) | #11 | Settings file | `phase-1/11-settings-file` |
| ✅ Done (#40) | #17 | Save a custom prompt as a reusable preset | `phase-2/17-custom-prompt-preset` |
|  | #18 | Integration tests with a mocked AI provider | `phase-2/18-mock-ai-tests` |
| In review (#44) | #27 | Bring your own key: multiple AI platforms with automatic key detection | `phase-2/27-multi-key-detection` |
| In review (#59) | #28 | OpenAI (GPT) provider | `phase-2/28-openai-provider` |
| In review (#60) | #29 | Google Gemini provider | `phase-2/29-gemini-provider` |
| In review (#61) | #30 | OpenAI-compatible platforms: OpenRouter, NVIDIA and others | `phase-2/30-openai-compatible` |
| In review (#62) | #32 | OpenCode Go provider | `phase-2/32-opencode-go` |
| ✅ Done (#47) | #20 | Theme toggle and keyboard navigation pass | `phase-3/20-theme-keyboard` |
|  | #22 | v1.0 acceptance pass | `phase-4/22-acceptance-pass` |
| ✅ Done (#43) | #23 | Download page and user guide | `phase-4/23-download-page` |

## Other finished issues

Workflow issues and follow-ups from code reviews, outside the two tracks.

| Status | Issue | Title | PR |
| --- | --- | --- | --- |
| ✅ Done | #1 | Repo workflow: PR template and issue-linked PRs | #26 |
| ✅ Done | #33 | Update build plan for multiple AI platforms | #34 |
| ✅ Done | #36 | Split open issues between the two contributors | #35 |
| ✅ Done | #48 | Show split marks on thumbnails | #51 |
| ✅ Done | #54 | Keyboard and theme fixes from the review of #47 | #55 |
| ✅ Done | #56 | Pass each thumbnail its finished mark (review of #51) | #57 |

## Hand-offs between tracks

Some issues wait on other work. Start with work that has no blockers, and rebase on `main` once the blocker merges.

| Waiting issue | Needs first |
| --- | --- |
| #64 Smart mode AI flow (@yawdeals1) | #7 page text, #8 run_preset, #14 excerpts (@yawdeals1) |
| #22 acceptance pass (@iamromelalvin7) | everything in phases 0 to 3 |

Unblocked starting points: Caleb on #7, #12 and #14, then #8 once #7 lands. @iamromelalvin7 on finishing the open provider PRs (#44, #59 to #62), and #18, which is now unblocked. #18 can build its mock on `ai::Provider` and test `ai::response::split`, which takes a provider.

## Working on your branch

```sh
git fetch origin
git switch phase-0/4-file-picker   # your issue's branch
git rebase origin/main             # before you start and before you open the PR
```
