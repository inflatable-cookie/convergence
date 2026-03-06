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

Documentation is the source of truth:
- Overview: [docs/README.md](/Users/betterthanclay/Dev/projects/convergence/docs/README.md)
- Vision: [docs/vision/001-convergence-platform-vision.md](/Users/betterthanclay/Dev/projects/convergence/docs/vision/001-convergence-platform-vision.md)
- Architecture + semantics: [docs/architecture/README.md](/Users/betterthanclay/Dev/projects/convergence/docs/architecture/README.md)
- Operator notes: [docs/operators/README.md](/Users/betterthanclay/Dev/projects/convergence/docs/operators/README.md)
- Process guardrails: [docs/processes/README.md](/Users/betterthanclay/Dev/projects/convergence/docs/processes/README.md)
- Roadmaps: [docs/roadmaps/README.md](/Users/betterthanclay/Dev/projects/convergence/docs/roadmaps/README.md)
- Logs: [docs/logs/README.md](/Users/betterthanclay/Dev/projects/convergence/docs/logs/README.md)

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

TUI with agent trace (JSONL semantic events):

```bash
cargo run --bin converge -- --agent-trace /tmp/converge-agent-trace.jsonl
```

Generate a friction report from a trace:

```bash
node scripts/agent-trace-report.js /tmp/converge-agent-trace.jsonl --out /tmp/converge-friction-report.md
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
