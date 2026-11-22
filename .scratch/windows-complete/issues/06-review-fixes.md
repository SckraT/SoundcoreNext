# 06 — Review fixes and release

**Status:** ready-for-agent
**Blocked by:** 05

## Goal

Review the completed Windows client and close the integration branch.

## Requirements

1. Run the code-review skill on `integration/windows-complete` against master
   (standards + spec axes); record findings.
2. Fix all hard findings and accepted judgement calls in one implementer pass.
3. Re-run desktop/core fmt, check, clippy, tests, build; update README/docs to
   match actual behavior (no overclaims).
4. Ensure no .po/reference artifacts or secrets are committed; ignore local
   scratch preview validation where appropriate.
5. Merge `integration/windows-complete` into `master` with a merge commit; clean
   ticket worktrees; report final commit and verification.

## Acceptance

- [ ] Review findings resolved or documented.
- [ ] Gates green on merged master.
- [ ] README and UI_KIT/GUIDE accurate.
- [ ] No local WIP/untracked production files left.
