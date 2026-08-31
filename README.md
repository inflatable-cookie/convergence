# Convergence

Convergence is an experimental next-generation version control and collaboration system.

Core idea: capture work continuously (or via explicit snapshots), then converge it through configurable, policy-driven gate stages into increasingly consumable bundles, culminating in release channels where appropriate.

Key terms:
- `snap`: a snapshot of a workspace state (not necessarily buildable)
- `publish`: submit a snap to a gate/scope as an input
- `bundle`: output produced by a gate after coalescing inputs
- `promote`: move a bundle to the next gate
- `release`: public or organizational output cut from an allowed gate
- `superpositions`: conflicts preserved as data and resolved per gate policy

## Current State

The g01-era implementation is archived at tag `v0-legacy` and branch
`archive/g01`. `main` carries the rebuilt stack: CLI, TUI, single-process
server, Postgres/S3 backends, gate graph, identity, secrets, git interop, and
semver releases. Terminology is **candidate** (not bundle) after `g02.029`.

Capture artifacts from the archived generation:

- [docs/rebuild/001-lessons-retrospective.md](docs/rebuild/001-lessons-retrospective.md)
- [docs/rebuild/002-tui-ux-spec.md](docs/rebuild/002-tui-ux-spec.md)
- [docs/rebuild/003-salvage-inventory.md](docs/rebuild/003-salvage-inventory.md)

Active generation: `g02` (29 roadmaps, closing). See
[docs/roadmaps/g02/README.md](docs/roadmaps/g02/README.md).

Documentation is the source of truth:
- Overview: [docs/README.md](docs/README.md)
- Vision: [docs/vision/001-convergence-platform-vision.md](docs/vision/001-convergence-platform-vision.md)
- Architecture + semantics: [docs/architecture/README.md](docs/architecture/README.md)
- Roadmaps: [docs/roadmaps/README.md](docs/roadmaps/README.md)
- Logs: [docs/logs/README.md](docs/logs/README.md)

## Effigy-First Loop

```bash
effigy tasks
effigy doctor
effigy health
effigy validate
effigy qa:docs
```

Rust 2024 edition. Direct commands when needed:

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo nextest run -P ci
```

## Next task

Run `g02.030` card 101, the isolated Northstar instruction and Rust quality
audit. Product direction remains paused: `g02.027` awaits the operator's TUI
cold-drive verdict and `g02.022` batch 22.5 awaits release authority. See
[docs/roadmaps/g02/README.md](docs/roadmaps/g02/README.md).
