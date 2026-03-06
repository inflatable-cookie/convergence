# Agents Operating Guardrails

Purpose: keep AGENTS guidance concise and operational while moving detail into docs.

## Working Model

- Treat `docs/` as source of truth for vision, architecture, logs, and roadmap intent.
- Keep implementation changes scoped; avoid unrelated refactors.
- Keep roadmap checklists current as tasks are completed.

## Source of Truth

- Overview: `docs/README.md`
- Vision: `docs/vision/`
- Architecture and semantics: `docs/architecture/`
- Log and rationale history: `docs/logs/`
- Delivery plans: `docs/roadmaps/`
- Operator guidance: `docs/operators/`

## Tooling Rules

- Use `cargo` for Rust checks and tests.
- Keep commands deterministic and repository-local.
- Do not introduce additional package managers/toolchains unless requested.

## Validation Baseline

Run what matches scope:

- `cargo check`
- `cargo fmt`
- `cargo clippy --all-targets -- -D warnings`
- `cargo nextest run -P ci` (for broader verification)

## Contract and Naming Rules

- Keep CLI/server behavior aligned with documented terms and workflows.
- Prefer explicit, typed interfaces over ad-hoc payload/command shapes.
- Keep naming consistent with existing docs and command vocabulary.

## Reporting and Roadmap Hygiene

- Track major migrations in the active `docs/roadmaps/g01/` sequence with checkboxes.
- Record rationale, doctrinal updates, and meaningful execution batches in `docs/logs/YYYY-MM/`.
- Mark completed tasks immediately.
- Keep updates factual: changed, validated, remaining.

## AGENTS File Principle

AGENTS should include only:

- scope
- hard rules
- minimal validation commands
- links to detailed docs

If guidance becomes long-form, move it into docs and link to it.
