## Summary

<!-- What this PR does, in one or two sentences. -->

Closes #

## Requirements

<!-- SRS/PRD IDs this PR implements, e.g. FR-30, NFR-05. -->

## How to test

1.

## Checklist

- [ ] One issue only; extra work goes in a new issue
- [ ] `cargo test --manifest-path src-tauri/Cargo.toml` passes
- [ ] `npm run build` passes
- [ ] Build plan item ticked in both `CLAUDE.md` and `AGENTS.md` (files still identical)
- [ ] `src/types.ts` updated if a serialized Rust type changed

## Security

- [ ] No secrets, keys, tokens, `.env` files or certificates in the diff
- [ ] No changes to environment variables, PATH or system settings; no machine-specific paths
- [ ] API keys stay in the OS credential store and are never logged (NFR-05)
- [ ] No new network calls outside the AI strategy and update check (NFR-06)
- [ ] The original PDF is never modified (FR-04)
