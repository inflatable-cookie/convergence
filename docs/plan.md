# Plan

Updated: 2026-09-26

## Now

1. **First tagged release** (lane `first-release`) — the release pipeline is
   built and no version has been cut. Operator-gated. Follow
   [release](knowledge/contracts/release.md). Open: Q-001.
2. **TUI cold-drive closeout** (lane `tui-closeout`) — navigation, guidance,
   and decisions-on-screen work is in; formal closeout waits on an operator
   verdict. Open: Q-002.

## Next

- Sweep removed process records for rulings that never landed in an owning
  knowledge file.
- Post-release redrive on the shipped surface once a tag exists.

## Not now

- Workflow profiles — needs a named design partner.
- Edge and horizontal scale — needs measured write or locality pain.
- Manifest paging for directories over 4096 entries — needs measured cost on
  real trees; see architecture 16.
- Encrypted secret names and hardware-backed keys — wait on deployment demand.
- Async candidate builds — wait on measured publish latency.
- Distributed control plane (architecture 14 §7) — after beachhead evidence
  and a measured single-process ceiling.
- Product repair from the Rust-quality audit — not authorized.
