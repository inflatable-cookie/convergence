# Convergence

Convergence is an experimental next-generation version control and collaboration system.

Core idea: capture work continuously (or via explicit snapshots), then converge it through configurable, policy-driven gate stages into increasingly consumable candidates, culminating in releases where appropriate.

Key terms:
- `snap`: a snapshot of a workspace state (not necessarily buildable)
- `publish`: submit a snap to a gate/scope as an input
- `candidate`: output produced by a gate after coalescing inputs (`g02.029` renamed it from `bundle`)
- `promote`: move a candidate to the next gate
- `release`: a candidate designated for consumption, identified by a semver version (`g02.028` retired channels)
- `superposition`: a conflict preserved as data and resolved per gate policy

## Current state

The g01-era implementation is archived at tag `v0-legacy` and branch
`archive/g01`. `main` carries the rebuilt stack: CLI, TUI, single-process
server, Postgres/S3 backends, gate graph, identity, secrets, git interop, and
semver releases. Terminology is **candidate** (not bundle) after `g02.029`.
No tagged product release has been cut.

Documentation is the source of truth:

- Overview: [docs/README.md](docs/README.md)
- Knowledge: [docs/knowledge/README.md](docs/knowledge/README.md)
- Plan: `plan.md` (Git history)
- Vision: [docs/knowledge/vision.md](docs/knowledge/vision.md)
- Architecture: [docs/knowledge/architecture/README.md](docs/knowledge/architecture/README.md)

Rebuild capture artifacts, including the TUI UX spec:

- [docs/rebuild/001-lessons-retrospective.md](docs/rebuild/001-lessons-retrospective.md)
- [docs/rebuild/002-tui-ux-spec.md](docs/rebuild/002-tui-ux-spec.md)
- [docs/rebuild/003-salvage-inventory.md](docs/rebuild/003-salvage-inventory.md)

## Commands

```bash
effigy tasks
effigy doctor
effigy health
effigy validate
effigy qa
```

Rust 2024 edition. Direct commands when needed:

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo nextest run -P ci
```
