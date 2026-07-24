# 007 Lanes and Collaboration

Status: active
Owner: repo maintainers
Updated: 2026-07-24



## Context

Lanes are currently a free string on publications. The operator confirmed
lanes stay in the vision and get built properly: the breadth/visibility
partition that carries unpublished work between collaborators and gives
superposition variants real provenance.

## Execution Plan (batch details in cards)

- **7.1 Lane model and registry**: server-side lane registry per repo
  (id, owner, members, visibility); lane ACLs wired into the capability
  grants; client `lane` verbs (create/list/join)
- **7.2 Unpublished sync**: push/pull snap lineage to a lane head without
  publishing (the "share WIP without the gate" story); lane heads carry
  lineage, not single snaps
- **7.3 Inbox**: reintroduce the g01 inbox as the triage surface — incoming
  lane activity and publications awaiting action, in CLI (`inbox`) and TUI
  (view + recommended actions)
- **7.4 Provenance tightening**: superposition variant sources become
  registered lane refs; publication provenance links lane + publisher +
  base

## Exit Criteria

- two clients share unpublished work through a lane with ACLs enforced;
  inbox surfaces it; variant provenance names registered lanes

## Next Task

Execute the ready Batch 7.3 card (`batch-cards/024-inbox.md`).
