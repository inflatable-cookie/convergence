# AGENTS

Scope: whole `convergence/` repository.

## Hard Rules

- Keep AGENTS content lean: scope, hard rules, validation, links.
- Treat `docs/` as source of truth for vision, architecture, roadmap intent, and rationale history.
- Keep roadmap checklists in sync with completed implementation work.
- Keep terminology consistent (`snap`, `publish`, `candidate`, `promote`, `release`, `superposition`).
- Do not recreate retired `docs/roadmap/` or `docs/decisions/` folders.

## Effigy-First Execution

Route by job, not startup ritual:

- `effigy tasks` — selector inventory
- `effigy doctor` — routing ambiguity or repo health
- `effigy graph` — code understanding (ownership, flow, changed-file impact)
- `effigy test --plan` — test shape before test-focused work (`cargo nextest run -P ci`)

Prefer `effigy <task>`, `effigy test`, and built-in surfaces over raw Cargo or
Node when Effigy covers the path. Use `effigy --json <command>` when another
agent or tool will consume output.

Direct commands only when the operation is not in `effigy.toml`.

## Validate

- `effigy health` — narrow baseline
- `effigy validate` — merge-ready Rust suite
- `effigy qa:docs` — docs and planning surfaces (required when docs change)

## References

- `docs/README.md`
- `docs/vision/001-convergence-platform-vision.md`
- `docs/architecture/README.md`
- `docs/roadmaps/g02/README.md`
- `docs/logs/README.md`
- `docs/specs/README.md`
- `docs/contracts/001-working-rules.md`
- `docs/contracts/contract-index.md`

## Strict Continuation Rule

- In the active strict lane, `continue` should resolve through the previous
  `Next Task`.
- If there is an active ready batch card, execution should anchor on that card.
- If there is no ready card, stop in planning instead of improvising execution.
- When the next move is materially ambiguous, ask for intent instead of
  guessing.

## Internal Writing Style

Use the repo-local style reference for internal work and normal replies:

- `docs/policy/internal-writing-style.md`

<!-- BEGIN EFFIGY AGENT CONTRACT -->
## Effigy Agent Contract

This repo's local `.agents/skills/effigy` copy is authoritative for this
project. When an agent supports both project-local and global skills, prefer
the project-local copy over any globally installed Effigy skill.

Do not add a `--repo` flag pointing at the current directory while already
inside the target repo. Do not edit
`.github/workflows/` or run release mutations unless the user explicitly asks.

Reference docs:
- Effigy agent adoption: `docs/guides/047-agent-and-cross-repo-adoption.md`
- Graph workflows: `docs/guides/076-code-graph-and-agent-workflows.md`
- JSON contracts: `docs/guides/017-json-output-contracts.md`
<!-- END EFFIGY AGENT CONTRACT -->
