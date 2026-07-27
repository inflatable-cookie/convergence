# Backlog

Backlog items are real candidate milestones not yet scheduled into a
roadmap.

The 2026-07-24 set has now been laid out as roadmaps (2026-07-25), so
this file no longer duplicates them:

- **Real identity** → [`g02/021-real-identity.md`](../g02/021-real-identity.md),
  ready
- **Workflow profiles** → [`g02/024-workflow-profiles.md`](../g02/024-workflow-profiles.md),
  parked on a design partner
- **Edge nodes** → [`g02/025-edge-and-scale.md`](../g02/025-edge-and-scale.md),
  parked on measured demand

Still here, unscheduled:

- **Gate graph administration**: `repo create` provisions one `intake`
  gate and nothing can change the graph afterwards — no CLI verb, and
  `/api/repos/:repo/gates` is read-only. This is not a trigger-based
  deferral like the items below it: `promote` is one of the six contract
  verbs and no user can reach it, so staged review, required approvals
  and a release-only final gate are all implemented and unusable
  (batch 22.4, finding 33). The write path is the easy half. Partition
  state is keyed by gate, windows advance per gate, and publications sit
  in a gate's open window, so removing or re-parenting a gate with live
  bundles is a data-migration question first. Wants a roadmap, not a card

- **Manifest paging**: sub-manifest pages for directories over 4096
  entries (doc 16 §1b). Worth stating precisely, because the backlog
  previously read as a correctness risk and is not one: the 4096 cap is
  on wire batch frames, which clients already split, so a large
  directory works today and is merely a large manifest. This is an
  efficiency deferral. Trigger: measured cost against real trees
- **Encrypted secret names** (doc 19 §9): the server sees names today.
  Trigger: a deployment where the existence of a credential is
  sensitive. Cost: listing requires decrypting every entry
- **Hardware-backed keys** (doc 19 §9): OS keychain, Secure Enclave,
  YubiKey. Trigger: a user asking to keep the private key off disk

## Next Task

Nothing to schedule from here: the live candidates are roadmaps in
`g02/`, and everything remaining is waiting on a stated trigger.
