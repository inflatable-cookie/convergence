# 2026-07-24 Audit Findings and the g02.011-g02.018 Hardening Program

Four independent adversarial audits ran against main at `5e2a82b`
(server security, client/sync/git correctness, UX, architecture/docs/
tests). This record is the canonical index of findings; roadmaps
`g02.011`-`g02.018` cite these ids. Verdict: the semantic core
(base-aware merge, lineage identity, deterministic bundles, provenance
verify, git interop refusals) is sound and tested; the failures cluster
in unfinished server trust boundaries, destructive client restore
paths, doc-14-vs-code drift, and workflow dead ends.

## Server security (audit A) — routed to g02.011, g02.012, g02.013

- **C1** Read endpoints skip authorization: `get_object`, `get_batch`,
  `negotiate`, `get_bundle`, `get_provenance`, `verify_bundle`
  authenticate only. Shared object store ⇒ any read grant discloses
  every repo's content by hash. Fix needs reachability/ownership
  check; objects are not repo-tagged. → 11.1
- **C2** GC collects reachable data: only guard for uploaded-but-
  unpublished objects is a hard-coded 300s mtime grace; publish checks
  root manifest only. Upload→publish spans over 5 minutes lose blobs
  silently. Needs pin/pending mechanism. → 12.2
- **C3** Personal-lane squatting: `personal/*` unreserved; attacker-
  owned `personal/victim` captures default publishes and locks the
  victim out. → 11.2
- **H1** Promote lacks monotonicity/base guards; stale bundle rewinds
  `window_floor`, re-opening consumed publications. → 13.2
- **H2** No transactions anywhere in either backend; publish/promote
  are multi-lock read-modify-writes; global mutex serializes
  statements, not operations; Postgres auto-commits. → 13.1
- **H3** `verify` (GET) writes merge outputs into the live object
  store; unauthorized write amplification. → 11.3
- **H4** Merge decision-table hole: modify-back-to-W vs delete
  collapses to a clean delete instead of superposing (doc 17 breach).
  → 13.3
- **M1** `delete_releases_for_bundles` matches releases by JSON
  `LIKE '%bundle_id%'` — over-deletes, cascading into GC sweeping live
  objects. → 13.4
- **M2** Unbounded batch endpoints; no body limit; memory
  amplification DoS. → 11.4
- **M3** Free-string `scope_id`; no scope registry; unbounded
  partition minting; typo'd scopes silently never merge. → 14.3
- **M4** `put_snap` accepts records with absent root manifests →
  dangling lane heads. → 12.2
- **L1** Static plaintext token map, non-constant-time lookup, no
  expiry/revocation. → 14.1 (doc honesty) + backlog (real identity)
- **L2** `add_lane_member` outside the `authorize` discipline. → 11.2
- **L3** Snap-id parent join not length-prefixed (latent). → 13.4
- **L4** Raw `anyhow` chains leak internals cross-repo. → 11.3
- **L5** `require()` capability gate is tautological — not real
  defense-in-depth. Noted; no card.
- **L6** No pagination on any list endpoint; inbox O(gates × window).
  → 15.2
- **L7** Object id charset unvalidated before FS/bucket paths
  (mitigated by routing + verify-on-read). Noted; folded into 11.4.

## Client/sync/git correctness (audit B) — routed to g02.012, g02.013

- **D1** `restore_snap` clears the workspace before materializing;
  superposed/unfetchable targets leave it emptied. → 12.1
- **D2** Path traversal: `materialize_manifest` joins untrusted entry
  names/symlink targets unvalidated. → 12.1
- **D3** Torn snapshots under concurrent writes: silent small-file
  truncation vs hard large-file abort at the 8 MiB threshold. → 12.4
- **D4** Thinning degrades lineage ordering past gaps (cosmetic).
  Noted; no card.
- **D5** `restore --force` is the only recovery path after D1 and is
  itself destructive. Subsumed by 12.1.
- **cC1** `validate_resolution` misses superpositions nested in Dir
  variants; validate passes, apply fails. → 13.4
- **cC2** Unlocked read-modify-write of `state.json`; lost updates
  stale the merge base pointer. → 13.4
- **cC3** Recapture holes: message dropped on unchanged tree; no dedup
  without HEAD; `put_snap` overwrite discards metadata. → 13.4
- **cC4** `upload_tree` prune trusts "manifest ⇒ subtree" and uploads
  negotiate-ordered; interrupts orphan subtrees permanently. → 12.3
- **cC5** `pull_lane` swallows transient errors as thinned gaps;
  truncated lineage looks authoritative. → 12.3
- **G1** fast-import paths unquoted; newline/quote filenames corrupt
  the stream. → 18.3 (caught by filename fuzzing; fix with it)
- **G2** git-map saved non-atomically after the ref moves; crash ⇒
  duplicate commits, permanent divergence. → 12.4
- **G3** Empty dirs dropped on export (git limitation, no `.gitkeep`
  synthesis). Noted; backlog candidate.
- **G4** Import worktree debris on interruption; cleanup errors
  swallowed. Noted; folded into 12.4.
- **R1** `write_atomic` never fsyncs; power loss can zero state files.
  → 12.4
- **R2** `read_config` writes as a side effect on the hot path. → 12.4

## Architecture/docs/tests (audit C) — routed to g02.014, g02.015, g02.018

- **1.1** Bundle builds synchronous in the publish request; `Building`
  never constructed; docs 14 §5 / 16 §1 claim async. → 14.2
- **1.2** Both backends global single-writer mutex; doc 14 §1/§3 claim
  partitioned no-global-locks. → 14.1 (doc) + 13.1 (transactions);
  true partitioned writes stay target-architecture
- **1.3** Edge nodes entirely unbuilt. → 14.1 (explicit deferral)
- **1.4** `snap-sync` capability specified, absent. → 11.2
- **1.5** Tokens static/in-memory vs "short-lived capability-scoped".
  → 14.1 (doc) + backlog
- **1.6** GC global cross-repo scan vs "partition-scoped, never
  stop-the-world". → 14.4
- **2.1** Full-window remerge, no Merkle short-circuit; doc 17 §2
  "bounded by changed paths" currently false; quadratic per window.
  → 15.1
- **2.2** `manifest_has_superpositions` re-walks full tree per
  publish. → 15.1
- **2.3** Publish not atomic (= H2). → 13.1
- **2.4** No scope registry; `scope_pattern` literal equality (= M3).
  → 14.3
- **2.5** TUI rebuilds full client stack per 3s tick and keystroke.
  → 15.3
- **Events table unbounded**; nothing prunes; gap-recovery poll can
  return full history. → 14.4
- **Test gaps**: zero concurrency, zero failure injection, zero
  large-tree, property tests only for chunking, zero pathological
  filenames, zero TUI reducer tests, live external backends never
  exercised. → 18.1-18.4 (+ per-roadmap regression tests)

## UX (audit D) — routed to g02.016, g02.017

- **P1 dead ends**: `resolve apply` orphan manifest id (16.1); inbox
  "resolve" recommendation unroutable (16.1); `sync pull` needs
  undiscoverable `restore --force` (16.2); `fetch` without `--into`
  invisible (16.2); TUI in uninitialized dir silent dead end (17.1);
  no bootstrap/member verbs — onboarding impossible (16.3).
- **P2 spec breaches**: six of ten spec views missing + palette gaps
  (17.1); synchronous loads on UI thread — spec §7.1 wart
  reintroduced (17.2); no idle refresh/timestamps, event refresh
  skips inbox (17.2); approve/promote/release/gc unconfirmed (17.3);
  resolution view lacks live validate + Alt+f/Alt+n (17.3); remote
  dashboard unranked; workflow profiles unbuilt (17.4 decision).
- **P3 output quality**: raw JSON in TUI Last strip (17.3); `{:?}`
  Debug leaks in CLI (16.4); message-flag inconsistency (16.4);
  publish wizard hardcodes `--lane default` (17.3); bundle-id arity
  inconsistency (16.4); `watch --json` breaks envelope (16.4).
- **P4 missing affordances**: no `show <snap>` browser — History
  Enter is destructive restore (16.2); no undo/`unsnap` (16.2); no
  transfer progress (16.4); no TUI help/Settings, watch not in TUI
  (17.1); no reachability signal (17.2).
- **Minor**: substring suggestions, missing cursor keys, trace event
  gaps, Tab context oddities (17.4).

## Decision

Hardening program `g02.011`-`g02.018` opened, sequenced by dependency:
trust boundaries → data safety → transactional/merge correctness →
architecture honesty → scale walls → workflow completion → TUI spec
parity → adversarial test hardening. 016 may run parallel to 014/015
after 013. Backlog unchanged (manifest paging, workflow profiles
pending 17.4 decision, edge nodes, real identity, SSE). Active
roadmap: `g02.011`; next move is batch card 11.1.
