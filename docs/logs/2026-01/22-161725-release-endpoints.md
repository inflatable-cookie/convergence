# Decision: Releases Can Be Cut From Non-Terminal Gates

Timestamp: 2026-01-22 16:17:25

## Context

Earlier docs described a release as something produced only from the terminal/final gate of the primary gate graph.

In practice, organizations may need to cut releases from earlier phases for reasons like:
- compatibility maintenance for older versions
- feature-flagged distributions
- emergency patches that intentionally bypass later-phase checks

## Decision

- A `release` is a bundle designated for consumption via a named release channel.
- The default and most common release endpoint is the terminal gate of the primary gate graph.
- The system may allow releasing from non-terminal gates, as long as:
  - gate policy allows it
  - permissions allow it
  - provenance is recorded for the release action

## Consequences

- Release creation becomes a first-class policy surface ("who can cut which channel from which gate").
- "Final gate" remains the recommended default for public/mainline releases, but does not constrain other organizational needs.
