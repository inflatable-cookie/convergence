# 009 Git Interop

Status: planned
Owner: repo maintainers
Updated: 2026-07-24

Opens after `g02.008`.

## Context

Nobody migrates to a VCS cold. Convergence must live alongside git: the
beachhead teams keep their git history and tooling while Convergence owns
the binary-heavy and gated workflows. Operator-confirmed first-class vision
item (was archived g01 doc 11).

## Planned Batches

- **9.1 Interop architecture**: mapping contract (snap lineage <-> commits,
  bundles <-> merge commits, tombstones <-> deletions), boundary rules
  (what Convergence owns vs mirrors), promoted to `docs/architecture/`
- **9.2 Export**: mirror snap lineage / released bundles to a git branch
  (fast-import stream); round-trip byte fidelity tests
- **9.3 Import**: seed a workspace from an existing git repo (history depth
  configurable); ignore-file translation
- **9.4 Coexistence mode**: `.git` and `.converge` in one tree; capture
  excludes git internals; docs for the side-by-side workflow

## Exit Criteria

- a real git repo imports, works under Convergence, and exports a mirror
  branch git tooling can consume

## Next Task

Compile into batches with a ready card when `g02.008` closes.
