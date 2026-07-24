# 021 Interactive Views

Status: ready
Updated: 2026-07-24
Roadmap: `g02.006`
Spec: `docs/specs/006-continuous-capture-and-workspace-ux.md`

## Objective

The TUI's selection half: act on the selected item, edit snap messages,
resolve by stable variant key.

## In Scope

- history view selection (Up/Down when input empty) with selected-item
  verbs: Enter -> action menu or direct restore prompt (confirmed),
  `d` diff selected vs head; selection surfaced to the agent trace as
  focused element
- CLI `annotate <snap> <message>` verb (message edit post-capture,
  identity-stable per doc 17); TUI `m` on selected snap opens a one-field
  wizard for it
- resolution decisions keyed by variant key (stable across variant
  reordering) instead of index in the decisions file the TUI writes;
  digit keys still pick visually
- reducer tests for selection movement, selected-item actions, annotate
  flow

## Out Of Scope

- new server behavior

## Acceptance Criteria

- history is navigable and actionable without typing ids; annotate works
  end to end; TUI-written decisions files use variant keys; suites green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Stop Conditions

- needs CLI semantics that do not exist — extend CLI first (annotate is
  the planned extension)

## Next Task

On completion, close roadmap `g02.006` against its exit criteria.
