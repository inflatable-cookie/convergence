# AGENTS

Scope: whole `convergence/` repository.

## Hard Rules

- Keep AGENTS content lean: scope, hard rules, validation, links.
- Treat `docs/` as source of truth for architecture, semantics, and roadmap intent.
- Keep roadmap checklists in sync with completed implementation work.
- Keep terminology consistent (`snap`, `publish`, `bundle`, `promote`, `release`).

## Validate

- `cargo check`
- `cargo fmt`
- `cargo clippy --all-targets -- -D warnings`
- `cargo nextest run -P ci` (when broader verification is needed)

## References

- `docs/processes/260-agents-operating-guardrails.md`
- `docs/architecture/README.md`
- `docs/roadmap/`
