# Phase 028: TUI Simplification (Kid-Friendly UX)

## Goal

Make the TUI feel non-overwhelming and "obvious" without reading docs by:
- reducing visible command surface area in each view
- adding a clear default action per context
- keeping advanced functionality discoverable via a single "full palette" gesture (`/`)

## Tasks

### A) Local mode

- [x] Rename commands to kid-friendly nouns (`save`, `history`), keep `publish`.
- [x] Make `Enter` with empty input run a safe default action (save/open history).
- [x] Reduce view chrome text that enumerates many commands.

### B) Remote mode

- [x] Keep remote commands single-name (no extra aliases).
- [x] Make `Enter` with empty input run a safe default action (open inbox; fetch in list views).
- [x] Reduce view chrome text that enumerates many commands.

### C) Discoverability

- [x] Make `/` open a full palette for the current mode (root + mode commands).

## Exit Criteria

- Each view surfaces at most 1-2 "hint" actions.
- `Enter` does the obvious safe thing in list views.
- Advanced commands remain accessible via `/`.
