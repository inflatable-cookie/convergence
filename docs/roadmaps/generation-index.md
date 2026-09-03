# Roadmap Generation Index

- `g02`
  - Status: active (closing)
  - Range: `001` to `031`
  - Notes: |
    Post-research planning gate (001), archive-and-rebuild boundary
    (002), rebuild vertical slice (003), TUI rebuild (004), semantics
    revision (005), continuous capture and workspace UX (006), lanes and
    collaboration (007), releases/retention/GC (008), git interop (009),
    scale and transport (010) — all complete.
    Audit hardening program (011-018, opened 2026-07-24 from the
    four-part audit) — complete (closed 2026-07-25).
    Secret substrate program (019-020) — complete (2026-07-25).
    Identity suite (021), ship readiness (022), TUI completion (023) —
    021 and 023 complete; 022 in progress (22.5 release pipeline built,
    release not cut, operator-gated).
    Workflow profiles (024) and edge/scale (025) — parked on triggers.
    Gate administration (026), TUI usability (027), semver releases
    (028), candidate rename (029) — 026, 028, 029 complete; 027 closing
    (cards 096, 097, 100 complete; operator cold-drive verdict pending).
    Northstar instruction and Rust quality audit (030) — complete.
    Installed Rust package consumer canary (031) — complete as an independent
    evidence-only maintenance lane; card 102.
    Next roadmap ID: `g02.032` if more g02 work is needed before rollover.

- `g01`
  - Status: complete
  - Range: `001` to `046`
  - Notes: |
    Foundational Convergence milestone sequence (001-041),
    Northstar doctrine alignment (042),
    and Comparative Research Program (043-045) — Complete.
    Optional research expansion (046) remains lineage, not the live queue.
    Roadmap files archived on branch `archive/g01` (docs spine restructure,
    2026-07-23).

## Strategic horizons (atlas, 2026-08-17)

Long-horizon runway for Convergence. Milestone detail stays in generation
roadmaps; this section names phases and rollover conditions only.

### H1 — Close g02 and ship

Outcome: a trustworthy first release and closed usability debt from the
rebuild.

- Operator verdict on TUI cold-drive usability (`g02.027` closeout)
- First tagged release (`g02.022` batch 22.5) when the operator authorises
  the cut
- Post-release redrive on the shipped surface (pattern from 22.4 / 27-28 logs)
- Formal closeout of open g02 roadmaps before generation rollover

Unlocks: external operators can install without a toolchain; vocabulary frozen
at candidate + semver.

### H2 — Beachhead adoption

Outcome: real teams using Convergence where git is weakest (binary-heavy small
teams: DAW, game assets, VFX).

- Operator guide and shakedown feedback loop into bounded roadmaps
- Workflow profiles (`g02.024`) when a design partner asks by name
- Git interop exercised on non-toy repos; coexistence pain surfaced early
- Determinism and provenance story validated with adopters (vision constraint)

Non-goals: guessing profile vocabulary; enterprise gate governance before
beachhead proof.

### H3 — Measured scale and platform depth

Outcome: ceilings proven before architecture expands.

- Edge/horizontal scale (`g02.025`) only on measured write or locality pain
- Manifest paging efficiency (backlog) on measured cost
- Encrypted secret names, hardware-backed keys (backlog) on deployment demand
- Async candidate builds only if publish latency is measured as painful

Rollover trigger for strategy review: a real workload hits the single-process
server ceiling documented in architecture doc 14.

### H4 — g03 generation (sketch, not open)

Outcome: architecture doc 14 §7 target — distributed control plane, partitioned
data plane, honest multi-process operation — without pretending the shipped
single-process server is already that system.

Depends on: g02 closeout, beachhead evidence, measured scale triggers.

Sketch themes (to become g03 roadmaps when rollover opens):

- partition workers and build queues with crash semantics
- coordination across processes over one partition
- read-through caches with promotion-safe invalidation
- large-org gate governance at scale (growth story, not entry story)

Do not open `g03` until g02 rollover closeout conditions in working rules
are satisfied.

## Rollover policy

Create a new generation only when maintainers explicitly decide the sequencing baseline needs a real reset.

Generations should be substantial. As a healthy default, expect something closer to 20 to 40 roadmap files before rollover is worth discussing. Treat that as a judgment guardrail, not an automatic counter.

Rollover is a closeout event, not a convenience move. Before opening the next generation:

- close, pause, supersede, or rehome every roadmap in the current generation
- refresh the roadmap front doors so the old generation is visibly closed
- purge stale generation-specific specs from `docs/specs/` so the active planning tree no longer carries dead lane debris

If that cleanup has not happened, stay in the current generation and finish the closeout there first.
