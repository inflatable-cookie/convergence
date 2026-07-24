# 020 Workspace Status

Status: ready
Updated: 2026-07-24
Roadmap: `g02.006`
Spec: `docs/specs/006-continuous-capture-and-workspace-ux.md`

## Objective

One canonical workspace-status verb; bundle status moves aside; the TUI
consumes the new verb wholesale and surfaces watcher/capture state.

## In Scope

- `converge status`: pending change count + lines, head snap (id, message,
  trigger), snap counts (explicit/automatic), remote target + last-seen
  bundle + last-published snap; single JSON object
- bundle status moves to `converge bundle <id>` (old `status <bundle>`
  removed — pre-1.0, no aliases)
- TUI root view renders from the status verb only (drop the piecemeal
  changes/history/remote calls for root data); header shows capture info
- tests: status verb happy path + fields through the binary; TUI reducer
  data-shape test updated

## Out Of Scope

- interactive views (6.3)

## Acceptance Criteria

- one CLI call answers "where am I" completely; TUI root uses it; all
  suites green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- status needs data no client API exposes — extend `converge-client`
  deliberately, not ad hoc

## Next Task

On completion, open the Batch 6.3 interactive-views card.
