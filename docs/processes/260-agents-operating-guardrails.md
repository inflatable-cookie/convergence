# Agents Operating Guardrails

Purpose: keep AGENTS guidance concise and operational while moving detail into docs.

## Working Model

- Treat `docs/` as source of truth for architecture, decisions, and roadmap intent.
- Keep implementation changes scoped; avoid unrelated refactors.
- Keep roadmap checklists current as tasks are completed.

## Source of Truth

- Architecture and semantics: `docs/architecture/`
- Decision history: `docs/decisions/`
- Delivery plans: `docs/roadmap/`
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

- Track major migrations in `docs/roadmap/` with checkboxes.
- Mark completed tasks immediately.
- Keep updates factual: changed, validated, remaining.

## AGENTS File Principle

AGENTS should include only:

- scope
- hard rules
- minimal validation commands
- links to detailed docs

If guidance becomes long-form, move it into docs and link to it.
