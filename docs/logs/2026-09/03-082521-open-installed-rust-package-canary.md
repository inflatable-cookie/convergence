# Open Installed Rust Package Canary

Date: 2026-09-03
Roadmap: `g02.031`
Card: `102`
Status: ready for worker dispatch

Northstar core PR 27 merged as
`256d0f78892e5cc22fa1672431fe8310df4f9162`, promoting the official Rust
package at registry version `1.4.0`. Convergence remains the selected real
consumer: its current `main` is
`1f05db1e507aa67f73a68eccc2325e23dfc1d478`, and its existing profile and
deviation hashes match the inventory recorded before package publication.

Card 102 bounds the proof to installed lifecycle, both workflow scopes,
pre-extraction evidence compatibility, Rust-only inventory, forced visible
fallback, and documentation closeout. Product repair and release work remain
outside the lane.

## Next Task

Dispatch card 102 from its committed handoff and stop for exact-head review.
