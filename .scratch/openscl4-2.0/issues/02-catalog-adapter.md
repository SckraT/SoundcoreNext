# 02 — Catalog adapter

**Type:** task
**Status:** ready-for-agent
**Blocked by:** —

## Goal

Turn `soundcore_models::catalog` into the data the vendored Add Device dialog expects, without changing the Slint UI definition.

## Files

- `desktop/Cargo.toml` (add `soundcore-models` path dependency)
- `desktop/src/catalog_adapter.rs` (new)
- `desktop/src/main.rs` (only if a `mod catalog_adapter;` declaration is needed; ticket 03 owns the wiring)

## Requirements

1. A public `CatalogEntry` (or equivalent) with `code`, `name`, `kind: i32`, and a `slint::Image` art.
2. Mapping from `FormFactor` to the vendored UI category indexes:
   - `TwsEarbuds` → 1 (Earbuds)
   - `Headphones` → 2 (Over-ear)
   - `OpenEar` → 3 (Open-ear)
   - `Speaker` → 4 (Speakers)
   - `SleepBuds` → 1 (Earbuds; the vendored UI has no Sleep chip)
3. Art selection from the vendored `ui/art` SVGs via `slint::include_image!`, one image per form factor. Use existing OpenSCL4 art; do not draw new assets.
4. `filter_models(query: &str, kind: i32) -> Vec<ModelOption>`:
   - `kind == 0` means All;
   - case-insensitive match on code or name;
   - stable order: form factor, then family, then catalog order;
   - empty query returns all in that kind.
5. Exactly the 39 `CATALOG` entries are exposed; unsupported codes never appear.
6. Unit tests for mapping, count, search by code/name, empty result and kind filtering. `ModelOption` is a Slint type, so test the pure `CatalogEntry`/filter layer before conversion.

## Acceptance

- [ ] `cargo test` in `desktop` passes.
- [ ] `cargo clippy --all-targets -- -D warnings` passes.
- [ ] No UI `.slint` file is edited.
- [ ] No network or protocol code is added.
