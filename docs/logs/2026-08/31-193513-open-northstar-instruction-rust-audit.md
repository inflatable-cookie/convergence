# Open Northstar Instruction And Rust Audit

The operator selected Convergence as the first project in a project-by-project
Northstar instruction and language-quality audit program.

## Decision

- Convergence has no active Paseo orchestrator, so this thread owns the worker
  and PR review loop.
- The repository is Rust-only: a root workspace plus five member crates.
- The lane uses Northstar's repository-scope Rust explicit audit and its
  target-aware AGENTS review in one serial worker PR.
- Product execution remains paused. This maintenance lane is isolated as
  `g02.030`; it does not borrow the release or TUI closeout queues.
- Existing doctor god-file and attention-marker findings are evidence leads,
  not blanket cleanup authority.

## Planning Evidence

- `main` was clean and equal to `origin/main` at
  `7bd11279d9894bac4bbc16c4ba7f63c0c6c2e19d`.
- Northstar Rust activation, strict profile, deviations file, and all six Cargo
  manifests are present.
- `effigy tasks` resolves the repository validation surface.
- `effigy doctor` reports the existing god-file error and attention-marker /
  stale-graph warnings; the audit must not misstate those as new or silently
  widen into a structural rewrite.

## Next Task

Dispatch card 101 through the committed worker handoff, then review the PR.
