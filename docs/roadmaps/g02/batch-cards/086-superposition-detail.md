# 086 Superposition Detail

Status: complete
Updated: 2026-07-25
Roadmap: `g02.023`

## Objective

Stop asking people to choose between file contents the screen will not
show them.

## Scope of the actual problem

Spec §6 specified a 65/35 list+detail split for Superpositions, and
batch 17.4 deferred it with the trigger "variants carrying more than a
source and a size". That reads like polish.

Batch 23.1 drove the flat list and found it is not. The screen renders
`docs/plan.md  [2 variants]  undecided` and offers `1-9 pick`. There is
no way to see what either variant contains. The person resolving a
conflict is choosing blind, and the resolution then lands as a snap.

## In Scope

- a bounded per-variant preview from the CLI, so the view loads it
  through the argv contract like everything else
- the 65/35 split, with the detail pane numbered to match the pick keys
- honest non-answers: binary, deleted, not-local

## Out Of Scope

- a real diff between variants: this is "what am I choosing between",
  not "what changed"
- editing a variant in place

## Acceptance Criteria

- both versions legible before choosing; previews bounded in bytes and
  lines; unpreviewable variants say what they are; the decisions file is
  unchanged

## Validation

- `effigy validate`
- `effigy qa:docs`

## Outcome

- `converge resolve list <ref> --preview` returns each variant's key
  *plus* a bounded look at its content. The key is untouched, so the
  decisions file keeps the shape `resolve apply` already expects — the
  preview is additive rather than a second format
- bounded twice: `PREVIEW_BYTES` caps what is read (a chunked file reads
  only its first chunk, so the bound holds on the store as well as the
  output) and `PREVIEW_LINES` caps what is shown
- **refusing to guess is the feature.** A binary gets no text; it gets
  `binary, 12 bytes`. Driving it is what forced the size in: two
  variants labelled only "binary" are not a choice, and the size is
  usually the thing that tells a 4.1 MB render from a 4.3 MB one. A
  tombstone reads "deleted in this variant", because a deletion is a
  real option somebody has to be able to pick
- the detail pane numbers variants to match the `1-9` keys that select
  them, and marks the current pick
- **the dashboard's own recommendation did not work.** Batch 23.4 made
  `Enter` on Root run the top-ranked action, and for a superposed bundle
  that argv is `resolve list <id>` — which ran as a raw command and
  printed "2 superposed path(s)" into the Last strip instead of opening
  the view. The inbox row had the right mapping all along; it was
  private to that one call site. Now shared as `action_for_argv`
- 276 tests green

## Next Task

`g02.023` is complete. Remaining scheduled work: `g02.022` ship
readiness, still planned and unscheduled.
