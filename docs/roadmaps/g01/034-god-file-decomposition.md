# Phase 034: God-File Decomposition Program (Consolidated)

## Goal

Track the full god-file reduction program in one place while preserving a clear, searchable summary of what was completed.

## Scope

This consolidated phase replaces the former per-wave roadmap files and captures the same initiative end-to-end:
- Legacy Phase 034 (`034-god-file-decomposition.md`)
- Legacy Phase 036 (`036-tui-and-core-god-file-wave-2.md`)
- Legacy Phases 037-109 (`037-god-file-decomposition-wave-3.md` through `109-god-file-decomposition-wave-75.md`)

## Program Outcomes

- Large single-file modules were split into focused submodules across TUI, CLI, remote client, and server handlers.
- Ownership boundaries were made explicit (orchestration vs. parsing vs. IO vs. rendering).
- Behavior preservation was prioritized over semantic redesign.
- Validation discipline (fmt/clippy/tests) was applied throughout decomposition waves.

## Consolidated Workstreams

### A) TUI decomposition
- `src/tui_shell/app.rs` decomposed into focused command/event/render/state modules.
- `src/tui_shell/wizard.rs`, `src/tui_shell/status.rs`, and view modules split by flow and concern.

### B) CLI decomposition
- `src/main.rs` reduced to argument surface and dispatch entrypoints.
- Execution logic moved into domain modules under `src/cli_exec/`.

### C) Server decomposition
- `src/bin/converge-server.rs` reduced to bootstrap/router composition.
- Domain handlers, persistence, validation, and shared helpers moved into focused modules.

### D) Remote client decomposition
- `src/remote.rs` split into transport, identity, transfer, fetch, operations, and typed DTO modules.

### E) Hygiene and verification
- Formatting, linting, and regression checks were run repeatedly while waves landed.
- Wave-by-wave details remain available through git history.

## Legacy Wave Index

The following legacy files are now consolidated into this single phase document:
- `036-tui-and-core-god-file-wave-2.md`
- `037-god-file-decomposition-wave-3.md`
- `038-god-file-decomposition-wave-4.md`
- `039-god-file-decomposition-wave-5.md`
- `040-god-file-decomposition-wave-6.md`
- `041-god-file-decomposition-wave-7.md`
- `042-god-file-decomposition-wave-8.md`
- `043-god-file-decomposition-wave-9.md`
- `044-god-file-decomposition-wave-10.md`
- `045-god-file-decomposition-wave-11.md`
- `046-god-file-decomposition-wave-12.md`
- `047-god-file-decomposition-wave-13.md`
- `048-god-file-decomposition-wave-14.md`
- `049-god-file-decomposition-wave-15.md`
- `050-god-file-decomposition-wave-16.md`
- `051-god-file-decomposition-wave-17.md`
- `052-god-file-decomposition-wave-18.md`
- `053-god-file-decomposition-wave-19.md`
- `054-god-file-decomposition-wave-20.md`
- `055-god-file-decomposition-wave-21.md`
- `056-god-file-decomposition-wave-22.md`
- `057-god-file-decomposition-wave-23.md`
- `058-god-file-decomposition-wave-24.md`
- `059-god-file-decomposition-wave-25.md`
- `060-god-file-decomposition-wave-26.md`
- `061-god-file-decomposition-wave-27.md`
- `062-god-file-decomposition-wave-28.md`
- `063-god-file-decomposition-wave-29.md`
- `064-god-file-decomposition-wave-30.md`
- `065-god-file-decomposition-wave-31.md`
- `066-god-file-decomposition-wave-32.md`
- `067-god-file-decomposition-wave-33.md`
- `068-god-file-decomposition-wave-34.md`
- `069-god-file-decomposition-wave-35.md`
- `070-god-file-decomposition-wave-36.md`
- `071-god-file-decomposition-wave-37.md`
- `072-god-file-decomposition-wave-38.md`
- `073-god-file-decomposition-wave-39.md`
- `074-god-file-decomposition-wave-40.md`
- `075-god-file-decomposition-wave-41.md`
- `076-god-file-decomposition-wave-42.md`
- `077-god-file-decomposition-wave-43.md`
- `078-god-file-decomposition-wave-44.md`
- `079-god-file-decomposition-wave-45.md`
- `080-god-file-decomposition-wave-46.md`
- `081-god-file-decomposition-wave-47.md`
- `082-god-file-decomposition-wave-48.md`
- `083-god-file-decomposition-wave-49.md`
- `084-god-file-decomposition-wave-50.md`
- `085-god-file-decomposition-wave-51.md`
- `086-god-file-decomposition-wave-52.md`
- `087-god-file-decomposition-wave-53.md`
- `088-god-file-decomposition-wave-54.md`
- `089-god-file-decomposition-wave-55.md`
- `090-god-file-decomposition-wave-56.md`
- `091-god-file-decomposition-wave-57.md`
- `092-god-file-decomposition-wave-58.md`
- `093-god-file-decomposition-wave-59.md`
- `094-god-file-decomposition-wave-60.md`
- `095-god-file-decomposition-wave-61.md`
- `096-god-file-decomposition-wave-62.md`
- `097-god-file-decomposition-wave-63.md`
- `098-god-file-decomposition-wave-64.md`
- `099-god-file-decomposition-wave-65.md`
- `100-god-file-decomposition-wave-66.md`
- `101-god-file-decomposition-wave-67.md`
- `102-god-file-decomposition-wave-68.md`
- `103-god-file-decomposition-wave-69.md`
- `104-god-file-decomposition-wave-70.md`
- `105-god-file-decomposition-wave-71.md`
- `106-god-file-decomposition-wave-72.md`
- `107-god-file-decomposition-wave-73.md`
- `108-god-file-decomposition-wave-74.md`
- `109-god-file-decomposition-wave-75.md`

## Exit Criteria

- A single roadmap file represents the god-file reduction initiative.
- Per-wave roadmap file sprawl is removed from `docs/roadmaps/g01/`.
- Roadmap numbering remains contiguous after consolidation.
