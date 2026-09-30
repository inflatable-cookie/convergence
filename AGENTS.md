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
- Tool and process friction: Queue, via `papercut.add` (see the
  `northstar` skill)

The plan (lanes, their documents and their order), leads, papercuts, brief
drafts, tasks and status live in Queue, never in this repository. Read what's
next with `plan.get` (see the `northstar` skill).

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
  lifecycle files, or routine logs. The plan, leads and tasks
  live in Queue.
- When a change alters what is true, update the owning knowledge file in the
  same PR.
- An operator ruling given in conversation goes into its owning file before
  the thread ends.
- Write in `docs/knowledge/contracts/writing-style.md`.

## Validate

Targeted and once per task: run the tests for the code you changed, compile
what you touched, and `effigy qa:docs` when docs changed. Full `effigy qa`
belongs to the planner on `main` at Queue milestones, not to each task.

- `effigy health` — narrow baseline
- `effigy validate` — merge-ready Rust suite
- `effigy qa:docs` — docs surfaces (required when docs change)

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

Effigy guidance is maintained in the installed shared Agent Skill. Read the
installed `effigy/SKILL.md` from one of these user skill roots when using
Effigy-specific agent guidance: `~/.agents/skills`, `~/.codex/skills`,
`~/.claude/skills`, or `~/.cursor/skills`. Resolve symlinks first; aliases to
the same canonical skill directory are one installation. If distinct roots
contain the skill, report the ambiguity and choose one source explicitly.

This repo's `.agents/skills/effigy` copy is optional project-local content,
not the maintained guidance source. Preserve it if it exists; plain init does
not create or refresh it. The named `skill.codex_project` init action is an
explicit snapshot opt-in and may replace files at maintained paths. Named
`effigy skill run` task lookup still gives an invocation project's local skill
source precedence, as defined by contract 042.

If no installed Effigy Agent Skill is present, say so and suggest
`npx skills add inflatable-cookie/effigy -g`; init does not download or install
skills. A filesystem check cannot prove what an already-running agent loaded;
use a fresh agent context to verify discovery.

Agent Skill guidance and the `effigy` executable are separate channels. A
current skill does not prove the binary on `PATH` is current or admission-capable;
check the binary independently with `command -v effigy` and
`effigy admission status --json`.

Do not add a current-directory repo override while already inside the target
repo. Do not edit
`.github/workflows/` or run release mutations unless the user explicitly asks.

Reference docs:
- Effigy agent adoption: `docs/guides/047-agent-and-cross-repo-adoption.md`
- Installed skill task sources: `docs/knowledge/contracts/042-external-skill-task-runner-contract.md`
- Heavy validation admission: `docs/guides/080-host-wide-validation-admission.md`
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
