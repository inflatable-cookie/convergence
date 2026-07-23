# 004 Docs Spine Restructure

Status: complete
Updated: 2026-07-23
Roadmap: `g02.002`
Spec: `docs/specs/002-archive-and-rebuild-boundary.md`

## Objective

Reduce `docs/` to the lean keeper spine from the salvage inventory; archive
the rest with the `archive/g01` branch as evidence.

## In Scope

- carry: vision, `architecture/01` + `04`, condensed product guardrails,
  research dossiers/memos/tracks, podcast summary, `docs/rebuild/`, live
  planning surfaces (contracts, specs, g02 roadmaps, logs)
- dedupe the object model (currently restated ~4x across vision/architecture/
  research); one canonical statement in architecture, others reference it
- remove from `main`: g01 roadmap files, remaining architecture docs
  (02-03, 05-12, prototypes), `operators/`, `processes/`, `testing/`,
  `git-podcast/` raw transcript, research scaffolding (templates, hubs,
  empty README wrappers)
- update remaining architecture docs to state the dev-server era is archived
  and Batch 2.4 owns the new server design
- realign front doors, indexes, and Effigy docs QA targets to the reduced tree

## Out Of Scope

- writing new architecture (Batch 2.4)
- code changes

## Acceptance Criteria

- every file under `docs/` is either a keeper, a live planning surface, or a
  log; nothing governs the archived implementation
- no broken links; docs QA green on the reduced tree

## Validation

- `effigy qa:docs`
- `effigy qa:northstar`

## Stop Conditions

- a doc marked archive turns out to carry unique durable substance — move the
  substance to a keeper first

## Outcome

- docs tree reduced 134 → 59 files; every remaining file is a keeper, live
  planning surface, or log
- removed: g01 roadmap files, architecture 02-03/05-12 + prototypes,
  operators/processes/testing, research scaffolding (templates, hubs,
  playbooks, crossrefs), podcast raw transcript + analysis set — all
  reachable on `archive/g01`
- rewrote architecture/research/git-podcast READMEs for the lean spine;
  architecture/01+04 are the canonical object model (dedupe satisfied by the
  cut — vision holds intent only, memo 003 stays as evidence)
- front doors, AGENTS references, and generation index updated; link check
  green

## Next Task

Execute the Batch 2.4 rebuild-architecture card
(`005-rebuild-architecture.md`).
