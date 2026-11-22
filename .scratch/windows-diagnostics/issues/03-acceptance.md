# 03 — Acceptance

**Status:** done
**Blocked by:** 02

## Evidence (2026-11-22, Windows 11 26H2, A3062 fw 04.39)

1. Diagnostics opens without a device and shows the empty state; opens with a
   connected Space One Pro from the sidebar.
2. **Start capture** on the real A3062 ran the three read-only probes:
   `State: Ok`, `Battery: Ok`, `Charging: Ok`; the page moved to Review with
   `3 of 3`, `6 frames`, `0 timeouts`, archive `1.2 KB`.
3. **Save ZIP** opened the native save dialog and wrote a real archive to
   `D:\Projects\SoundcoreNext\diag-test.zip` (1,199 bytes). Extracted contents:
   `manifest.json`, `frames.jsonl`, `README.txt`; manifest model `A3062`,
   probeCount 3, frameCount 6. The archive contains **no serial and no MAC**
   (`303432…` and MAC-shaped regexes absent) — the A3062 serial redaction in
   `core/crates/diagnostics/src/redact.rs` was fixed during acceptance and
   covered by a regression test.
4. Prefilled issue URL is built and covered by a unit test; nothing is uploaded
   by the app.
5. Screenshots: `desktop/diagnostics-page.png` (connected Device),
   `desktop/diagnostics-capture.png` (Review), `desktop/diagnostics-800.png`
   (adaptive check). All linked/described in `desktop/README.md`; no
   address/serial appears in the UI or captures.
6. Adaptive checks at 800×700, 1030×760, 1280×820 and 1440×900: 800 px uses the
   compact sidebar and one-column cards with no horizontal overflow; wider
   layouts keep the cards inside the client width (UIA bounds checked).
7. Gates: desktop 220 passed / 0 failed / 13 ignored; core 127+ diagnostics
   tests pass; fmt/clippy clean.
8. A3949 has no diagnostics capture path; its worker is not kept.

## Notes

- The vendored app shell is 125% scaled on this bench; the 1618×1072 capture is
  the full physical window.
- The phone pairing change from the previous dual-connections slice is unrelated.
