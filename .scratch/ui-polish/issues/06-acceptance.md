# 06 — Acceptance and release

**Status:** ready-for-agent
**Blocked by:** 01–05

## Requirements

1. Run the app (demo A3062 and real A3954) and verify:
   - breadcrumb arrow centered at 100/125/150%;
   - Settings has no Diagnostics block; Copy logs works from Diagnostics;
   - Effects page long descriptions/reasons do not cross dividers at 800×700 and
     150%;
   - LDAC quick-toggle reason is Russian; switch language and verify.
2. Regenerate the Russian visual matrix/contact sheets where useful; no clipping.
3. Run desktop/core fmt/build/test/clippy, including translation test.
4. Code review standards/spec; fix findings.
5. Merge into master; clean worktrees; report commits, gates, screenshots and any
   remaining limitations.

## Acceptance

- All four user-visible issues fixed; translation test enforced; master green.
