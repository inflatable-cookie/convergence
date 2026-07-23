# 002 Archive-and-Rebuild Boundary

Status: complete
Updated: 2026-07-23
Roadmap: `g02.002`

## Context

The post-research boundary decision (`g02.001` Batch 1.2) is made. Operator
intent, 2026-07-23:

- the concept is sound; the implementation is not the product
- the current server architecture cannot serve large distributed systems and is
  throwaway
- the local capture theory (cheap snaps, quality gate at publish) is validated
  and stays
- the TUI implementation sprawled, but its UX is good and must survive as a
  captured spec, not as carried code
- archive the current implementation, keep the lessons, restructure docs to a
  lean strict spine, then rebuild client and server as independent systems

## Boundary Decisions

- Repo shape: this repo, Cargo workspace — shared model crate + client crate +
  server crate. Independent systems, one repo.
- Archive mechanics: tag current state (`v0-legacy`), keep `archive/g01`
  branch, then strip `main` to the docs spine plus salvage.

## Salvage Verdicts

Carry forward:

- `src/model/` — blake3 content-addressed Merkle DAG with
  `ManifestEntryKind::Superposition` (conflict-as-data). Promote to shared crate.
- `src/store/` — verify-on-read, atomic writes, `write_if_absent` dedup.
- `src/diff/`, `src/resolve/` — small, model-driven, portable.
- Sync protocol shape (missing-objects negotiation → upload → publish) as a
  contract, not necessarily the code.

Discard:

- Entire server storage/state layer (in-memory RwLock maps, whole-repo
  `repo.json` rewrite persistence). Rebuild on real object store + metadata DB.
- Fixed 4 MB block chunking — replace with content-defined chunking.
- TUI implementation (16.5k LOC). UX survives as spec only.

Docs: ~10-15 of 134 files carry (vision, object model, superpositions,
research dossiers/memos/tracks, condensed guardrails, podcast summary). Rest
archives with the branch.

## Governing Refs

- `docs/contracts/001-working-rules.md`
- `docs/roadmaps/g02/002-archive-and-rebuild-boundary.md`
- `docs/roadmaps/generation-index.md`

## Lane Focus

- capture everything worth keeping before anything is cut
- archive honestly: nothing deleted from history, `main` stripped deliberately
- rebuild planning only after the spine is clean

## Batch Model

- one ready card at a time, in roadmap batch order
- capture batches complete before the archive cut executes
- rebuild execution gets its own roadmap files after architecture definition

## Capture Artifacts (Batch 2.1, complete)

- `docs/rebuild/001-lessons-retrospective.md`
- `docs/rebuild/002-tui-ux-spec.md`
- `docs/rebuild/003-salvage-inventory.md`

## Current State

- Batch 2.1 (capture) complete.
- Batch 2.2 (archive cut) complete: `v0-legacy` tag + `archive/g01` branch;
  `main` is docs + salvaged lib-only crate.
- Batch 2.3 (docs spine restructure) complete: docs tree 134 → 59 files,
  keeper spine only.
- Batch 2.4 (rebuild architecture) complete: `docs/architecture/13-16`
  promoted; operator decisions recorded (central control plane + partitioned
  data plane; one binary, pluggable storage backends).

## Exit Condition

This spec completes when `main` holds the lean docs spine plus salvaged crates,
the archive tag and branch exist, and rebuild architecture work has its own
governing surfaces.

## Next Task

Compile the first rebuild implementation roadmap (`g02.003`) from
architecture docs 13-16; archive this spec once `g02.003` opens.
