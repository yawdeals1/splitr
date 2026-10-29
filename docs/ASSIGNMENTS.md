# Work split

Who owns which open issue. Each issue already has its own branch on `origin`; check it out, do the work there, and open a PR that says `Closes #N`. The workflow rules in `CLAUDE.md` still apply: one issue per branch and PR, green CI, and the owner reviews and merges.

| Contributor | GitHub | Track |
| --- | --- | --- |
| Caleb Mensah (owner) | @yawdeals1 | PDF engine, core backend, AI core, security-sensitive and release work |
| Second contributor | @iamromelalvin7 | Frontend UI, settings, extra AI providers, tests, docs |

The split follows code areas, so the two tracks rarely touch the same files. Swap an issue by editing this file in a PR.

## Caleb (@yawdeals1)

| Issue | Title | Branch |
| --- | --- | --- |
| #2 | PDF engine: bundle and load PDFium | `phase-0/2-bundle-pdfium` |
| #3 | Open PDF: file info, password prompt and clear errors | `phase-0/3-open-pdf` |
| #6 | Export parts with lopdf | `phase-0/6-export-parts` |
| #7 | Extract page text and outline into DocumentText | `phase-1/7-extract-text` |
| #8 | run_preset command with fallback strategies | `phase-1/8-run-preset` |
| #12 | Test fixture set of 10+ PDFs | `phase-1/12-test-fixtures` |
| #13 | API key storage, AI toggle and consent notice | `phase-2/13-api-key-storage` |
| #14 | Page excerpts and overlapping chunks | `phase-2/14-excerpts-chunks` |
| #15 | Claude API client | `phase-2/15-claude-client` |
| #16 | AI response parsing, merging and validation | `phase-2/16-ai-response-parsing` |
| #31 | Connect a ChatGPT account (Sign in with ChatGPT) | `phase-2/31-chatgpt-signin` |
| #19 | Auto-update with signed update manifest | `phase-3/19-auto-update` |
| #21 | Installer size check and clean-machine install test | `phase-3/21-installer-size` |
| #24 | OCR for pages without a text layer | `phase-5/24-ocr` |
| #25 | Local AI model option | `phase-5/25-local-model` |

#19 needs the updater signing key in GitHub secrets, and #31 needs the owner to confirm OpenAI's developer terms, so both stay with the owner.

## @iamromelalvin7

| Issue | Title | Branch |
| --- | --- | --- |
| #4 | File picker and drag and drop | `phase-0/4-file-picker` |
| #5 | Lazy thumbnail grid and page viewer | `phase-0/5-thumbnail-grid` |
| #9 | Review and edit screen | `phase-1/9-review-screen` |
| #10 | Naming template, ZIP and open folder | `phase-1/10-naming-zip-open-folder` |
| #11 | Settings file | `phase-1/11-settings-file` |
| #17 | Save a custom prompt as a reusable preset | `phase-2/17-custom-prompt-preset` |
| #18 | Integration tests with a mocked AI provider | `phase-2/18-mock-ai-tests` |
| #27 | Bring your own key: multiple AI platforms with automatic key detection | `phase-2/27-multi-key-detection` |
| #28 | OpenAI (GPT) provider | `phase-2/28-openai-provider` |
| #29 | Google Gemini provider | `phase-2/29-gemini-provider` |
| #30 | OpenAI-compatible platforms: OpenRouter, NVIDIA and others | `phase-2/30-openai-compatible` |
| #32 | OpenCode Go provider | `phase-2/32-opencode-go` |
| #20 | Theme toggle and keyboard navigation pass | `phase-3/20-theme-keyboard` |
| #22 | v1.0 acceptance pass | `phase-4/22-acceptance-pass` |
| #23 | Download page and user guide | `phase-4/23-download-page` |

## Hand-offs between tracks

Some issues wait on the other track. Start with work that has no blockers, and rebase on `main` once the blocker merges.

| Waiting issue | Needs first |
| --- | --- |
| #5 thumbnails (@iamromelalvin7) | #2 PDFium, #3 open PDF (@yawdeals1) |
| #9 review screen (@iamromelalvin7) | #8 run_preset (@yawdeals1) |
| #10 naming, ZIP, open folder (@iamromelalvin7) | #6 export (@yawdeals1) |
| #17 custom prompt preset, #18 mocked AI tests (@iamromelalvin7) | #15 Claude client, #16 parsing (@yawdeals1) |
| #27 multiple keys (@iamromelalvin7) | #13 key storage (@yawdeals1) |
| #28, #29, #30, #32 providers (@iamromelalvin7) | #15 Claude client, which sets the provider interface (@yawdeals1) |
| #22 acceptance pass (@iamromelalvin7) | everything in phases 0 to 3 |

Unblocked starting points: Caleb on #2, @iamromelalvin7 on #4 and #11.

## Working on your branch

```sh
git fetch origin
git switch phase-0/4-file-picker   # your issue's branch
git rebase origin/main             # before you start and before you open the PR
```
