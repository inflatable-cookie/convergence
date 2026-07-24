# Roadmap Generation Index

- `g02`
  - Status: active
  - Range: `001` to `010`
  - Notes: |
    Post-research planning gate (001, complete), archive-and-rebuild
    boundary (002, complete), rebuild implementation vertical slice
    (003, complete), TUI rebuild (004, complete), convergence semantics
    revision (005, complete), continuous capture and workspace UX
    (006, complete), lanes and collaboration (007, active), then planned
    in sequence: releases/
    retention/GC (008), git interop (009), scale and transport (010).

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
