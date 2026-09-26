# Convergence

An experimental version control and collaboration system. Work is captured
continuously or explicitly, then converged through configurable gate stages
into artifacts a team can consume, with conflicts kept as data and decided
later rather than resolved at merge time.

The vocabulary is load-bearing; the wrong word is how this repository drifts.

- `snap` — a snapshot of a workspace tree. Not necessarily buildable.
- `publish` — submit a snap into a gate, in a scope, as an input.
- `candidate` — what a gate produces by coalescing its input publications.
  `g02.029` renamed it from `bundle`.
- `promote` — move a candidate to a downstream gate.
- `release` — a candidate designated for consumption, identified by a semver
  version. `g02.028` retired channels.
- `superposition` — a conflict preserved as data and resolved per gate policy.

Where code and knowledge disagree, knowledge wins until a decision moves them.

## Where things live

- Current state: `docs/README.md`
- Knowledge (one owner per fact): `docs/knowledge/README.md`
- Retired concepts, which must not come back: `docs/knowledge/retired.toml`
- Open questions: `docs/knowledge/questions.md`
- What's next: `docs/plan.md`
- Unresolved leads: `docs/triage/`
- Tool and process friction: `PAPERCUTS.md`

Tasks, briefs and status live in Queue, never in this repository.

## Commands

Route by job, not startup ritual:

- `effigy tasks` — selector inventory
- `effigy doctor` — routing ambiguity or repo health
- `effigy graph` — code understanding (ownership, flow, changed-file impact)
- `effigy test --plan` — test shape before test-focused work (`cargo nextest run -P ci`)

Prefer `effigy <task>`, `effigy test` and built-in surfaces over raw Cargo when
Effigy covers the path, and `effigy --json <command>` when another agent or tool
will consume the output. Direct commands only for what `effigy.toml` misses.

## Product rules

Each is a versioned or hashed contract that already has a guard. Changing one
is an operator decision:

- **Wire format** — `converge_model::WIRE_VERSION`. A server refuses an
  unknown major outright. Reads of an older field name are the exception and
  are always explicit: `serde(alias = "bundle_id")` and its siblings carry
  `g02.029`. Adding or dropping one is a compatibility decision, not tidying.
- **On-disk format** — `converge_model::format`. A store carries a stamp and
  both directions of mismatch are refused. Adding a file nobody older reads is
  not a bump; changing what an existing file means is.
- **Object identity** — snap and candidate ids hash content and lineage.
  Changing what goes into one renames every record that already exists.
- **MSRV** — declared once in the root `Cargo.toml`. Never assume a universal
  one: resolve `docs/knowledge/contracts/rust-quality-profile.json`.
- **The argv contract** — the CLI owns the semantics (architecture doc 15).
  TUI and agents drive those verbs, so no surface may show what a CLI cannot.

Secret values never enter the TUI: the input buffer echoes, submitted lines
replay, and the trace outlives the session. Such verbs hand over to a shell.

## Guardrails

- Process records stay out of the repository: no roadmaps, handoffs,
  lifecycle files, or routine logs. Intent lives in `docs/plan.md`; tasks
  live in Queue.
- When a change alters what is true, update the owning knowledge file in the
  same PR.
- An operator ruling given in conversation goes into its owning file before
  the thread ends.
- Write in `docs/knowledge/contracts/writing-style.md`.

## Validate

`effigy qa` before opening a PR. That runs the merge-ready Rust suite, docs
checks, and workflow lint.

- `effigy health` — narrow baseline
- `effigy validate` — merge-ready Rust suite
- `effigy qa:docs` — docs surfaces (required when docs change)

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
the repository-owned profile and deviations under `docs/knowledge/contracts/`; never
assume a universal MSRV. Re-enter at task start and coherent batch closeout.
Preserve unrelated work. A quality audit, no-slop pass, or audit-and-fix request
is explicit audit intent; never route it through everyday authoring.
<!-- northstar:rust-quality:end -->
