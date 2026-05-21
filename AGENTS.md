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
- `docs/specs/README.md`
- `docs/contracts/001-working-rules.md`

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

Use Effigy as the default command surface for supported project work.

Route by job, not by startup ritual:
- use `effigy graph` for code understanding
- use `effigy tasks` for selector inventory
- use `effigy doctor` for routing ambiguity or repo health
- use `effigy test --plan` when test execution shape matters

Use `effigy graph` when the job is code understanding: ownership, flow,
implementation, or changed-file impact. Do not insert graph into unrelated
deployment, state, docs, release, or direct task-execution work.

Prefer `effigy <task>`, `effigy test`, and the matching built-in surface over
raw package-manager or shell commands when Effigy covers the path. Use
`effigy --json <command>` whenever another agent or tool will consume output.

This repo's local `.agents/skills/effigy` copy is authoritative for this
project. When an agent supports both project-local and global skills, prefer
the project-local copy over any globally installed Effigy skill.

Do not add `--repo .` while already inside the target repo. Do not edit
`.github/workflows/` or run release mutations unless the user explicitly asks.

Reference docs:
- Effigy agent adoption: `docs/guides/047-agent-and-cross-repo-adoption.md`
- Graph workflows: `docs/guides/076-code-graph-and-agent-workflows.md`
- JSON contracts: `docs/guides/017-json-output-contracts.md`
<!-- END EFFIGY AGENT CONTRACT -->
