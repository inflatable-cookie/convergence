# 009 Git Interop

Status: active
Owner: repo maintainers
Updated: 2026-07-24



## Context

Nobody migrates to a VCS cold. Convergence must live alongside git: the
beachhead teams keep their git history and tooling while Convergence owns
the binary-heavy and gated workflows. Operator-confirmed first-class vision
item (was archived g01 doc 11).

## Execution Plan (batch details in cards)

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

Execute the ready Batch 9.3 card (`batch-cards/032-git-import.md`).
