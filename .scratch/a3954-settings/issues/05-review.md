# 05 — Review and release

**Status:** ready-for-agent
**Blocked by:** 04

## Goal

Review and merge the A3954 settings branch.

## Requirements

1. Run code review (standards + spec) against master.
2. Fix hard findings and accepted judgement calls.
3. Re-run desktop/core fmt/build/test/clippy; update docs (README, SOURCE if UI
   strings changed) and ticket records.
4. Merge `feature/a3954-settings` into master with a merge commit; clean
   worktrees; report commit and gates.

## Acceptance

- Review findings resolved; master green; release commit reported.
