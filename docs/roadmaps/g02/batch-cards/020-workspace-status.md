# 020 Workspace Status

Status: complete
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

## Outcome

- `converge status`: one JSON report — pending changes (count + lines),
  head (id/message/trigger), snap counts (explicit/automatic), remote
  (target, last-seen bundle, last-published snap)
- bundle record moved to `converge bundle <id>`; no aliases (pre-1.0)
- TUI: root views render from the status report alone; local root shows
  head + trigger and automatic-capture count with a `watch` pointer;
  remote root adds last-seen bundle; `bundle` classified remote for the
  async worker, `status` local
- status verb test through the binary; 62 workspace tests green

## Next Task

Execute the Batch 6.3 interactive-views card (`021-interactive-views.md`).
