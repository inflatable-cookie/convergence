# Convergence

Convergence is an experimental next-generation version control and collaboration system.

Core idea: capture work continuously (or via explicit snapshots), then converge it through configurable, policy-driven phase gates into increasingly "consumable" bundles, culminating in a release.

Key terms:
- `snap`: a snapshot of a workspace state (not necessarily buildable)
- `publish`: submit a snap to a gate/scope as an input
- `bundle`: output produced by a gate after coalescing inputs
- `promote`: move a bundle to the next gate
- `release`: final public output (often from the terminal gate)
- "superpositions": conflicts preserved as data and resolved per gate policy

Documentation is the source of truth:
- Architecture + semantics: `docs/architecture/README.md`
- Operator notes: `docs/operators/README.md`
- Process guardrails: `docs/processes/README.md`
- Roadmap phases: `docs/roadmap/`
- Decisions: `docs/decisions/`

## Build

Rust 2024 edition.

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo nextest run -P ci
```

## Run (client)

CLI help:

```bash
cargo run --bin converge -- --help
```

TUI:

```bash
cargo run --bin converge
```

Local quickstart (workspace):

```bash
converge init
converge snap "first snapshot"
converge history
```

## Run (server, dev)

Start a local dev server:

```bash
cargo run --bin converge-server -- --addr 127.0.0.1:8080 --data-dir ./converge-data
```

Then login from a workspace:

```bash
converge login --url http://127.0.0.1:8080 --repo test --token dev
```

For shared dev servers / first-admin bootstrap, see:
- `docs/operators/bootstrapping-and-identity.md`

TUI server setup:
- In the TUI, press `Tab` to switch to remote and use `/bootstrap` (first admin) and `/create-repo` (repo setup).
