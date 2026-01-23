# Phase 017: TUI Remote Browsers (Inbox/Bundles/Superpositions)

## Goal

Replace the current split-panel + scrollback-heavy remote UX with modal, dedicated browsers for high-volume remote tasks.

## Scope

In scope:
- `inbox` mode: browse publications.
- `bundles` mode: browse bundles.
- `superpositions` mode: browse conflicted paths for a bundle and drive resolution.
- Lazy HTTP: fetch only when entering a remote mode or running a remote command.

Out of scope:
- Server-side resolution storage.

## Tasks

### A) Inbox mode

- [x] List publications with selection + details.
- [x] Mode-local commands:
  - [x] `bundle`: fetch bundle for selected publication
  - [x] `fetch`: fetch selected snap
  - [x] `back`

### B) Bundles mode

- [x] List bundles with selection + details (promotable + reasons).
- [x] Mode-local commands:
  - [x] `approve`
  - [x] `promote [--to-gate ...]`
  - [x] `superpositions`: enter superpositions mode for selected bundle
  - [x] `back`

### C) Superpositions mode

- [x] List conflicted paths with selection.
- [x] Detail view shows variants + current decision + validation state.
- [x] Mode-local commands:
  - [x] `pick <n>` / `clear`
  - [x] `next-missing` / `next-invalid`
  - [x] `validate`
  - [x] `apply [--publish]`
  - [x] `back`

## Exit Criteria

- Remote workflows (approve/promote/resolve) are doable without relying on the scrollback as the main UI.
- Errors and results show in the status/notification area and do not destroy the active view.
