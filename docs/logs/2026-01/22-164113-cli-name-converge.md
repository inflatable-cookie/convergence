# Decision: CLI Binary Name is `converge`

Timestamp: 2026-01-22 16:41:13

## Context

Earlier docs used `cnv` as the CLI binary name.

`cnv` is short, but it can conflict conceptually (and potentially by name) with conversion tools.

## Decision

- The primary CLI binary name is `converge`.
- The interactive TUI is invoked by running `converge` with no args.
- The deterministic command form is `converge <action>`.
- To avoid the awkward `converge converge`, the gate operation that produces a bundle is named `bundle`:
  - `converge bundle` produces a `bundle` at a gate.

## Consequences

- Documentation and UX should use `converge` consistently.
- A shorter alias can be considered later, but should not be the canonical name.
