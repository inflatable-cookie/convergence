# Phase 023: TUI Releases Browser

## Goal

Make releases first-class in the TUI by adding a small browser that shows channels + latest bundle pointers, and enables quick `fetch --release` / `release` creation workflows.

## Scope

In scope:
- New `releases` TUI view (remote mode).
- List release channels with latest bundle id and timestamp.
- Actions from the view:
  - `fetch` the selected channel
  - `release <channel>` from selected bundle (reuse bundles mode)

Out of scope:
- Full release history per channel (we can add later).
- Editing/deleting releases.

## Tasks

### A) TUI view

- [x] Add `UiMode::Releases` and a `ReleasesView`.
- [x] Add remote root command `releases`.
- [x] Render channel list with latest release details.

### B) Actions

- [x] In releases mode: `fetch` fetches selected release channel.
- [x] In releases mode: `back` returns to root.

### C) Tests

- [x] Add a lightweight test for release list sorting/grouping helper (if extracted).

## Exit Criteria

- TUI remote root can open a releases view.
- View shows at least one channel when releases exist.
- `fetch` in the view fetches the selected release into the local store.
