# Decision: Use "Bundle" For Gate Outputs

Timestamp: 2026-01-22 16:29:33

## Context

Convergence previously used the term `package` for the output produced by a gate after coalescing inputs.

While accurate, "package" is heavily overloaded in software development (npm/pip/apt packages, "publish a package", etc.). This creates a high risk of confusion between Convergence gate outputs and language/ecosystem packaging.

## Decision

- Rename the gate output object from `package` to `bundle`.
- Update documentation and vocabulary accordingly:
  - `publish` submits a snap to a gate
  - `converge` produces a bundle
  - `promote` advances a bundle to the next gate
  - `release` designates a bundle for consumption via a release channel

## Consequences

- The term "bundle" becomes a primary domain noun and should be used consistently in CLI/TUI language.
- "Artifact" remains available for build outputs attached to releases (to avoid collisions with "bundle").
