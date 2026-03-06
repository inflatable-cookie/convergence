# Phase 006: Superposition Resolution MVP

## Goal

Allow a user to resolve superpositions (conflicts) in a bundle by making explicit, durable choices.

Output should be a new snap that:
- is deterministic from the set of resolution choices
- can be published as a new input
- produces a promotable bundle when policy allows

This phase is intentionally narrow: resolve by choosing one variant (or tombstone) per conflicted path.

## Scope

In scope:
- A resolution decision format (stored locally in `.converge/`) that records, per conflicted path, which variant was chosen.
- A CLI command to apply a resolution to a bundle root manifest and produce a new snap.
- Minimal TUI integration: choose a variant for a superposition and write/apply the resolution.
- Tests that prove resolved bundles become promotable.

Out of scope:
- Automatic merge strategies.
- 3-way merges.
- Rich diffs.
- Cross-bundle reuse of partial resolutions.

## Design Notes

- Resolution should operate on a specific bundle (and its `root_manifest`).
- A resolution is a pure transform: `(root_manifest, decisions) -> resolved_root_manifest`.
- Determinism rule: applying the same decisions to the same input manifest graph yields the same resolved root manifest id.

## Tasks

### A) Resolution data model

- [x] Add `Resolution` model:
  - bundle_id
  - root_manifest
  - created_at
  - decisions: `path -> variant_key`
- [x] Define `variant_key`:
  - for now: `index` into the superposition variant list
  - later: stable key (e.g. `{source_publication_id, kind, object_id}`)
- [x] Persist resolution files under `.converge/resolutions/<bundle_id>.json`.

### B) Apply resolution

- [x] Implement `apply_resolution(store, root_manifest, decisions) -> ObjectId`:
  - walk manifest tree
  - when encountering `Superposition`, replace it with the chosen `File/Dir/Symlink/Tombstone`
  - recompute manifests bottom-up to new ids
- [x] Add `converge resolve apply --bundle <id> [--message ...]`:
  - fetch bundle + manifest tree if missing
  - apply resolution
  - write new snap record
  - optionally `--publish` into current scope/gate

### C) TUI integration

- [x] Superpositions screen: add a "pick variant" action to record the decision for the selected path.
- [x] Add an "apply" action to generate a resolved snap.
- [x] Display which paths are already decided.

### D) Tests

- [x] Add e2e test:
  - create two conflicting publications
  - create bundle (non-promotable due to superpositions)
  - apply resolution
  - publish resolved snap
  - create new bundle (promotable)

## Exit Criteria

- A user can turn a non-promotable bundle (due to superpositions) into a promotable bundle via explicit resolution choices.
- Applying the same resolution twice yields the same resolved root manifest id.
