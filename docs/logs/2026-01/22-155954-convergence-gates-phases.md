# Decision: Convergence Phases, Gates, and Bundles

Timestamp: 2026-01-22 15:59:54

## Context

Convergence is a version control / collaboration system designed for large development organizations first, while remaining flexible enough for solo and small-team usage.

The core mental model is a tree/network that converges through multiple phases (gates) down to a single point: a public release. Importantly, there are multiple levels of "ready for consumption" along the way.

## Decisions

### 1) Central authority (GitHub-like)

- The system is centralized-by-default.
- A central server is authoritative for:
  - repo definitions
  - identity and access control
  - gate graph definitions
  - scopes (branch-like depth dimension)
  - published objects, bundles, and releases
  - provenance/audit history
- Clients are authoritative only for local, private snapshots until they publish.
- Small-team mode uses the same architecture (a lightweight single-node server can run locally or on a shared machine), with good caching/offline behavior.

### 2) Vocabulary: stop overloading "commit"

- `snap`: local snapshot of workspace state (no requirement that it builds/works). In v1, created explicitly via `converge snap`.
- `publish`: submit a selected `snap` to a gate+scope as an input artifact ("complete for this phase").
- `bundle`: the output artifact produced by a gate after it coalesces inputs.
- `promote`: advance a bundle to the next gate.
- `release`: a bundle designated for consumption via a release channel (typically from the terminal gate, but not strictly required).

### 3) Convergence model: gate graph + breadth/depth scoping

- The repository defines a **gate graph** (a DAG of gates) that converges into a terminal "release" gate.
- **Breadth (strands/lanes):** organizational subgraphs/lanes (team/area ownership) that control default visibility and subscription boundaries.
- **Depth (scopes):** a branch-like dimension for feature/milestone/release-train tracks that flow through the gate graph.

Practical invariant:
- A workspace is always viewing a tuple like `(scope, base bundle, overlays)`.

### 4) Superpositions are policy-scoped by gates

- Superpositions are first-class objects (conflicts as data), not workflow errors.
- Large-org-safe default: you do not see everyone’s in-progress state; visibility is bounded by lane/strand and what you explicitly subscribe to.
- Gates define "superposition breadth" (who/what you can observe) and "superposition depth" (which scope you’re operating within).

### 5) Gate behavior: always emit a bundle; policy determines pass/promote

Decision:
- A gate always emits a `bundle` when it converges selected inputs, even if that bundle contains unresolved superpositions.
- The gate (and/or downstream gates) defines what state a bundle must be in to pass (promotable) via policy.

Rationale:
- Avoid blocking work while still allowing strictness where it matters.
- Preserve the system’s core promise: don’t conflate synchronization/capture with integration/release discipline.

### 6) CLI + TUI contract

- Deterministic CLI commands for automation/AI/tools:
  - `converge snap`, `converge publish`, `converge status`, `converge diff`, `converge fetch`, `converge bundle`, `converge promote`, `converge release`, `converge resolve`, `converge restore`
  - add `--json` outputs early.
- `converge` with no args opens an interactive TUI for:
  - gate graph + scope navigation
  - incoming publications/bundles "inbox"
  - superposition browsing and resolution
  - promotion workflow (what is promotable, what policy blocks it)

### 7) MVP sequencing

- v0 (spec-first): formalize object model, gate/scopes semantics, and workflows (dev/integrator/release).
- v1: local content store + `snap/diff/restore` + server-side gate graph/ACLs + `publish` intake + `bundle` objects.
- v2: TUI built on the same deterministic CLI/API.
- v3: optional daemon/IDE integration for automatic capture and richer forensics.

## Consequences

- Gate definitions become a primary configuration surface for orgs (policy, permissions, checks, and promotion rules).
- Superpositions are preserved and visible by design, but must be constrained by lane/scope and controlled by gate policy to scale.
- "Ready for consumption" is phased: publish/bundle/release are distinct and should be treated distinctly in UX and governance.

## Open Questions (next)

- How to represent and resolve superpositions without turning the working tree into a cluttered set of alternate files.
- Exact authorization model:
  - who can publish to a gate
  - who can converge inputs and emit bundles
  - who can promote across gates
- What the minimal gate policy DSL looks like (or whether policies are external CI workflows).
- Whether bundles can be partially promotable (by path/area) or only whole-bundle.
