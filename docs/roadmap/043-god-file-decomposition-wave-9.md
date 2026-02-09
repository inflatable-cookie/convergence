# Phase 043: God-File Decomposition (Wave 9)

## Goal

Continue reducing high-LOC CLI and remote hotspots by splitting dense command and execution modules into focused, cohesive files without behavior changes.

## Scope

Primary Wave 9 targets:
- `src/cli_subcommands.rs` (~318 LOC)
- `src/cli_commands.rs` (~294 LOC)
- `src/cli_exec/identity.rs` (~262 LOC)

## Tasks

### A) Baseline and Boundaries
- [x] Capture target order and decomposition boundaries for CLI-focused modules.

Progress notes:
- Start with `cli_subcommands.rs` because it is enum-only and low risk to split by command family.
- Follow with `cli_commands.rs` then `cli_exec/identity.rs` using command-group boundaries.

### B) CLI Subcommand Enum Decomposition
- [x] Replace `src/cli_subcommands.rs` with a module directory split by command family.
- [x] Keep public re-exports stable for `main.rs` and existing consumers.

Progress notes:
- Introduced module directory and per-family files:
  - `src/cli_subcommands/mod.rs`
  - `src/cli_subcommands/release.rs`
  - `src/cli_subcommands/identity.rs`
  - `src/cli_subcommands/resolve.rs`
  - `src/cli_subcommands/remote.rs`
  - `src/cli_subcommands/gate_graph.rs`
  - `src/cli_subcommands/user_token.rs`
- Removed monolith `src/cli_subcommands.rs`.

### C) CLI Command Root Decomposition
- [x] Split `src/cli_commands.rs` into `commands/` modules grouped by domain.
- [x] Preserve clap metadata, aliases, and argument defaults exactly.

Progress notes:
- Replaced `src/cli_commands.rs` with module directory:
  - `src/cli_commands/mod.rs`
  - `src/cli_commands/local.rs`
  - `src/cli_commands/identity.rs`
  - `src/cli_commands/delivery.rs`
- Kept top-level command names/aliases (`mv`, `gates` / `gate-graph`) and option defaults by moving argument definitions into `clap::Args` structs consumed by the same command variants.
- Updated `src/cli_exec.rs` match arms to consume typed argument structs without changing downstream handler signatures.

### D) CLI Identity Exec Decomposition
- [x] Split `src/cli_exec/identity.rs` into focused helpers by concern.
- [x] Preserve JSON/text output parity and error behavior.

Progress notes:
- Replaced `src/cli_exec/identity.rs` with module directory:
  - `src/cli_exec/identity/mod.rs`
  - `src/cli_exec/identity/token_user.rs`
  - `src/cli_exec/identity/membership.rs`
  - `src/cli_exec/identity/session.rs`
- Preserved handler signatures consumed by `src/cli_exec.rs` and retained existing command output text/JSON paths.

### E) Verification and Hygiene
- [x] Run `cargo fmt`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo nextest run` (or document fallback if environment instability persists).
- [x] Keep this phase doc checkboxes and notes current as slices land.

Progress notes:
- Validation after `cli_subcommands` and `cli_exec/identity` decomposition:
  - `cargo fmt` passed
  - `cargo clippy --all-targets -- -D warnings` passed
  - `cargo nextest run` passed (`64 passed`, `0 failed`)
- Validation after `cli_commands` decomposition rerun:
  - `cargo fmt` passed
  - `cargo clippy --all-targets -- -D warnings` passed
  - `cargo nextest run` passed (`64 passed`, `0 failed`)
