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

## Current State: Archive-and-Rebuild

The g01-era implementation (CLI, TUI, dev server) is archived. The full tree
lives at tag `v0-legacy` and branch `archive/g01`. `main` carries the docs
spine plus the salvaged core library: content-addressed model, local store,
diff, resolution, and workspace modules.

Capture artifacts from the archived generation:

- [docs/rebuild/001-lessons-retrospective.md](docs/rebuild/001-lessons-retrospective.md)
- [docs/rebuild/002-tui-ux-spec.md](docs/rebuild/002-tui-ux-spec.md)
- [docs/rebuild/003-salvage-inventory.md](docs/rebuild/003-salvage-inventory.md)

Rebuild lane: `docs/specs/002-archive-and-rebuild-boundary.md` and roadmap
`g02.002`.

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

Intent checkpoint: pick the next `g02.005` owner (releases/retention/GC,
external backends, identity, edge nodes, or workflow profiles); the lane is
in planning with no ready card.002` Batch 2.4 rebuild-architecture card.
