# Soundcore Next — desktop catalog prototype

First Slint screen: grouped model catalog with a detail panel. It is a design
and taxonomy validation, not yet a device controller.

## Run

```powershell
cd desktop
cargo run
```

First build pulls Slint and takes a few minutes.

## What it shows

- Form factors in order: TWS, full-size headphones, open-ear, speakers, sleep.
- Families inside each form factor: Liberty flagship, Liberty NC, P/A, Life,
  Space, Sport, Q line, open-ear, speaker, sleep.
- Each model row: code, name, battery topology and a "проверить" marker for
  inferred assignments.
- Click a row: detail panel with family, tier, parser status and shared
  capabilities.

## Data

The screen reads `soundcore_models::catalog`; it does not touch the protocol
or the transport. The source of the groups is
`wiki/docs/protocol/model-taxonomy.md` in the research repository.

## Design

Colours and spacing follow the tokens in `wiki/docs/apps/design-system.md`
(warm background `#f5efea`, white cards, purple accent `#6c4fb3`). The real
desktop shell will later reuse the OpenSCL4 Slint views under GPL-3.0; this
prototype proves the grouped picker before that port.
