# Roadmap Generation Index

- `g02`
  - Status: active
  - Range: `001` to `025`
  - Notes: |
    Post-research planning gate (001), archive-and-rebuild boundary
    (002), rebuild vertical slice (003), TUI rebuild (004), semantics
    revision (005), continuous capture and workspace UX (006), lanes and
    collaboration (007), releases/retention/GC (008), git interop (009),
    scale and transport (010) — all complete.
    Audit hardening program (011-018, opened 2026-07-24 from the
    four-part audit): server trust boundaries (011), data safety (012),
    transactional and merge correctness (013), architecture honesty
    (014), scale walls (015), workflow completion (016), TUI spec
    parity (017), adversarial test hardening (018) — all complete
    (closed 2026-07-25).
    Secret substrate program (019-020, opened 2026-07-25): individual
    client-encrypted secrets (019), shared multi-recipient secrets
    (020) — both complete.
    Next suite (021-025, laid out 2026-07-25): real identity (021,
    ready), ship readiness (022), TUI completion (023); workflow
    profiles (024) and edge/horizontal scale (025) parked on triggers.

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


## Rollover policy

Create a new generation only when maintainers explicitly decide the sequencing baseline needs a real reset.

Generations should be substantial. As a healthy default, expect something closer to 20 to 40 roadmap files before rollover is worth discussing. Treat that as a judgment guardrail, not an automatic counter.

Rollover is a closeout event, not a convenience move. Before opening the next generation:

- close, pause, supersede, or rehome every roadmap in the current generation
- refresh the roadmap front doors so the old generation is visibly closed
- purge stale generation-specific specs from `docs/specs/` so the active planning tree no longer carries dead lane debris

If that cleanup has not happened, stay in the current generation and finish the closeout there first.
