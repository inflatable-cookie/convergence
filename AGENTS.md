# AGENTS

Scope: whole `convergence/` repository.

## What Convergence Is

An experimental version control and collaboration system. Work is captured
continuously or explicitly, then converged through configurable gate stages
into artifacts a team can consume, with conflicts kept as data and decided
later rather than resolved at merge time.

The vocabulary is load-bearing; the wrong word is how this repository drifts.

- `snap` — a snapshot of a workspace tree. Not necessarily buildable.
- `publish` — submit a snap into a gate, in a scope, as an input.
- `candidate` — what a gate produces by coalescing its input publications.
  Never "bundle": `g02.029` renamed it everywhere.
- `promote` — move a candidate to a downstream gate.
- `release` — a candidate designated for consumption, identified by a semver
  version. Never a "channel": `g02.028` retired those.
- `superposition` — a conflict preserved as data and resolved per gate policy.

`docs/` is the source of truth for vision, architecture, roadmap intent and
rationale. Where code and docs disagree, docs win until a decision moves them.

## What Must Survive A Change

Each is a versioned or hashed contract that already has a guard, because
breaking one silently is worse than failing loudly. Changing one is an
operator decision, not an implementation detail:

- **Wire format** — `converge_model::WIRE_VERSION`. Servers refuse unknown
  majors; there are no pre-1.0 compatibility shims.
- **On-disk format** — `converge_model::format`. A store carries a stamp and
  both directions of mismatch are refused. Adding a file nobody older reads is
  not a bump; changing what an existing file means is.
- **Object identity** — snap and candidate ids hash content and lineage.
  Changing what goes into one renames every record that already exists.
- **MSRV** — declared once in the root `Cargo.toml`. Never assume a universal
  one: resolve `docs/contracts/rust-quality-profile.json`.
- **The argv contract** — the CLI owns the semantics (architecture doc 15).
  TUI and agents drive those verbs, so no surface may show what a CLI cannot.

## Sharp Edges

- Do not recreate the retired `docs/roadmap/` or `docs/decisions/` folders.
  `docs/roadmaps/` and `docs/logs/YYYY-MM/` replaced them; a second copy
  splits the queue, and the stale half still looks authoritative.
- Keep roadmap checklists in sync with the implementation that closed them. A
  finished card still advertised as ready is how the next agent picks up work
  that is already done.
- Secret values never enter the TUI: its input buffer is echoed, submitted
  lines replay, and the trace outlives the session. Verbs that need a value
  are handed over to a terminal instead.

## Effigy-First Execution

Route by job, not startup ritual:

- `effigy tasks` — selector inventory
- `effigy doctor` — routing ambiguity or repo health
- `effigy graph` — code understanding (ownership, flow, changed-file impact)
- `effigy test --plan` — test shape before test-focused work (`cargo nextest run -P ci`)

Prefer `effigy <task>`, `effigy test` and built-in surfaces over raw Cargo when
Effigy covers the path, and `effigy --json <command>` when another agent or tool
will consume the output. Direct commands only for what `effigy.toml` misses.

## Validate

- `effigy health` — narrow baseline
- `effigy validate` — merge-ready Rust suite
- `effigy qa:docs` — docs and planning surfaces (required when docs change)

Done means the suite passes, the docs that govern the change say what it now
does, and the card and roadmap agree about what is left.

## References

- `docs/README.md` — the documentation map
- `docs/vision/001-convergence-platform-vision.md` — why the project exists
- `docs/architecture/README.md` — the object model, gates, and invariants
- `docs/roadmaps/g02/README.md` — the live queue and what is parked
- `docs/specs/README.md` — active strict planning and ready cards
- `docs/logs/README.md` — what was done, and the reasoning at the time
- `docs/contracts/001-working-rules.md` — how work starts, closes, and continues
- `docs/contracts/contract-index.md` — every other contract in force

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

<!-- northstar:rust-quality:start -->
## Northstar Rust Quality

Scope: Rust source, Cargo manifests, build files, tests, and directly related
documentation under this directory.

Use Northstar's strict everyday-authoring route for ordinary Rust work. Resolve
the repository-owned profile and deviations under `docs/contracts/`; never
assume a universal MSRV. Re-enter at task start and coherent batch closeout.
Preserve unrelated work. A quality audit, no-slop pass, or audit-and-fix request
is explicit audit intent; never route it through everyday authoring.
<!-- northstar:rust-quality:end -->
