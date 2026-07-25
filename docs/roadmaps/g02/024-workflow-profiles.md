# 024 Workflow Profiles

Status: parked — trigger not fired
Owner: repo maintainers
Updated: 2026-07-25

## Context

UX spec §4.6 describes profiles that reorder recommendations and rename
domain terms: release becomes "mastered mixdown" for a DAW, or
"build-ready pack" for game assets. Batch 17.4 built the narrow half —
profiles are settable, and shape the guidance shown on the remote
dashboard and in Help.

The expensive half is renaming domain nouns across every surface. It
multiplies every string by three profiles, and it has to cover the CLI
as well as the TUI or the two front-ends speak different languages to
the same person.

## Trigger

A design partner in one of the non-software profiles asking for it by
name. Guessing at a vocabulary nobody has asked for is how a product
ends up with three half-right dialects.

## Sketch (not a plan)

- profile-aware term table with one source of truth for CLI and TUI
- recommendation reordering per profile, with owner labels
- a way to see the underlying canonical term, so support conversations
  and docs stay possible across profiles

## Next Task

None. Revisit when the trigger fires.
