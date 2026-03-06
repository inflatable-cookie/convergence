# AGENTS

Scope: whole `convergence/` repository.

## Hard Rules

- Keep AGENTS content lean: scope, hard rules, validation, links.
- Treat `docs/` as source of truth for vision, architecture, roadmap intent, and rationale history.
- Keep roadmap checklists in sync with completed implementation work.
- Keep terminology consistent (`snap`, `publish`, `bundle`, `promote`, `release`, `superposition`).
- Do not recreate retired `docs/roadmap/` or `docs/decisions/` folders.

## Validate

- `cargo check`
- `cargo fmt`
- `cargo clippy --all-targets -- -D warnings`
- `cargo nextest run -P ci` (when broader verification is needed)

## References

- `docs/README.md`
- `docs/vision/001-convergence-platform-vision.md`
- `docs/architecture/README.md`
- `docs/roadmaps/`
- `docs/logs/`
- `docs/processes/260-agents-operating-guardrails.md`
