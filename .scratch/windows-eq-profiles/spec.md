# Spec: OpenSCL4 2.0 — Windows saved equalizer profiles

**Status:** ready for implementation
**Tracker:** local Markdown in `.scratch/windows-eq-profiles/`
**Integration branch:** `integration/windows-eq-profiles`
**Depends on:** custom equalizer bands (`master` 883dc03)

## Goal

Let the user save the current custom equalizer curve as a named profile, apply it later, delete it, and keep profiles across app restarts. Import/export and migration stay out.

## In scope

- Profile model: name + 8 visible bands, validation (non-empty name, 8 bands, range, unique names).
- JSON schema v1 for a profile file, atomic persistence in the app data directory.
- Controller: load at startup, `has-saved-profiles=true` for A3062, list `eq-profiles` with curve and in-effect flag, save/replace dialog, apply, delete.
- Applying a profile writes the custom `0xFEFE` curve and re-polls.
- A3062 only; A3949 deferred.

## Out of scope

- Import/export file/clipboard, legacy migration, profile export selection. The
  vendored **Import and export** panel is rendered on the editable A3062 page
  but stays inert: its buttons do nothing (this slice makes no `.slint` edits).
- Profiles for other models; HearID/DRC.
- Android/Linux.

## Ticket graph

| Ticket | Depends on | Outcome |
|---|---|---|
| `01-profiles-core.md` | — | Model, validation, JSON schema + tests |
| `02-profiles-controller.md` | 01 | Persistence + save/apply/delete UI wiring |
| `03-acceptance.md` | 02 | Real A3062 + restart persistence, screenshot |

## Acceptance criteria

1. Gates pass (desktop/core test, fmt, clippy, build).
2. A3062 Equalizer shows “Save profile”; saving a custom curve creates a named profile with curve preview and “Applied” when current.
3. Applying a profile writes one `03 87` custom curve, re-polls, and marks the profile in effect.
4. Deleting removes it after confirmation.
5. Profiles persist across app restart (JSON file in app data); invalid/missing file starts empty without crashing.
6. Duplicate name asks to replace; an empty name is rejected in the dialog. Invalid bands are rejected by the profile model and on document load; the page's band edits clamp to the measured range, so the page itself cannot produce an invalid band.
7. The vendored Import and export panel is visible but inert (its buttons do nothing); A3949 deferred.
8. Screenshot stored/linked; no MAC/serial.
9. A failed profile write shows the readable page error banner (and a stderr log) and leaves the saved list and document unchanged; an unreadable document at startup is logged to stderr and loads empty.

## Closure

Ticket closed when acceptance passes and `Status: done`; merge to `master` after ticket 03 and `code-review`.
