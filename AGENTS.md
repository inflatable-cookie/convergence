# AGENTS

Scope: whole `convergence/` repository.

## Hard Rules

- Keep AGENTS content lean: scope, hard rules, validation, links.
- Treat `docs/` as source of truth for vision, architecture, roadmap intent, and rationale history.
- Keep roadmap checklists in sync with completed implementation work.
- Keep terminology consistent (`snap`, `publish`, `bundle`, `promote`, `release`, `superposition`).
- Do not recreate retired `docs/roadmap/` or `docs/decisions/` folders.

## Effigy-First Execution

- Start with `effigy tasks`.
- Run `effigy doctor` when environment or task resolution is uncertain.
- Prefer `effigy health` for the narrow baseline.
- Prefer `effigy validate` before broader merge-ready checks.
- Prefer `effigy test --plan` before test-focused work; the repo task intentionally defaults to `cargo nextest` when available.
- Run `effigy qa:docs` when docs or planning surfaces change.
- Use direct Cargo or Node commands only when the needed operation is not represented in `effigy.toml`.

## Validate

- `effigy health`
- `effigy validate`
- `effigy qa:docs`
- `effigy test --plan` (for test-focused work)

## References

- `docs/README.md`
- `docs/vision/001-convergence-platform-vision.md`
- `docs/architecture/README.md`
- `docs/roadmaps/`
- `docs/logs/`
- `docs/processes/260-agents-operating-guardrails.md`
