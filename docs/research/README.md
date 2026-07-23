# Convergence Comparative Research

Evidence base for Convergence design: how existing systems handled the
problems Convergence targets, and what that implies for our design.

The research program (g01.043-045) is complete. Findings are the durable
asset; the program scaffolding (templates, hubs, playbooks, crossrefs) is
archived on branch `archive/g01`.

## Findings

- `specimen-dossiers/` — five systems, each with architectural bets,
  strengths, chronic pain points, and Convergence lessons:
  - **Git** — distributed, object store, explicit staging
  - **Mercurial** — distributed, revlog, phases for mutability
  - **Perforce Helix Core** — centralized, streams as gates, file locking
  - **Plastic SCM** — hybrid, semantic merge, visual branching
  - **Jujutsu** — distributed (Git-backed), conflicts-as-data, operation log
- `value-tracks/` — cross-system syntheses:
  continuous-capture-vs-explicit-commit, gate-based-workflows,
  conflict-preservation
- `translation-memos/` — research → Convergence design decisions:
  001 snap semantics (prototype first), 002 gate policy (prototype first),
  003 superposition-as-data (promoted into `docs/architecture/04`)

## Working Rule

Promote stable conclusions into `docs/architecture/` or contracts; keep
tentative findings here. A finding is promotable when it states the problem,
the evidence, the accepted tradeoffs, and what must be prototyped or measured
first.

## Next Task

Use these findings as evidence for the `g02.002` Batch 2.4
rebuild-architecture card — the Perforce dossier's centralized-fragility
warnings apply directly to the server design.
