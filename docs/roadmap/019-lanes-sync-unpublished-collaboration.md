# Phase 019: Lanes, Sync, And Unpublished Collaboration

Current status:
- In progress: server lane heads + endpoints; client/CLI/TUI sync and lanes fetch.

## Goal

Make "unpublished" work durable and collaboratively accessible without sending it into the gate pipeline.

Convergence should support:
- continuous (or frequent) backup of work-in-progress
- teammates discovering and fetching each other's unpublished snaps
- keeping "publish" as the high-signal act that submits work to gates/scopes

## Why This Phase Exists

Right now:
- local work is volatile unless it is published
- publishing conflates two responsibilities:
  - durability / backup
  - submission into the release pipeline

We need a first-class shared surface for unpublished work so that:
- teammates can grab and experiment with each other's snapshots immediately
- conflicts can exist as data (superpositions) before any gate policy pressure

## Scope

In scope:
- Server-supported lanes with per-user "head" pointers to unpublished snaps.
- `sync` as a local action: upload snap + objects and update your lane head.
- `lanes` discovery and `fetch --lane` retrieval.
- Server retention/GC rooted on lane heads (in addition to pinned bundles and promotion state).
- TUI affordances for lane discovery + syncing.

Out of scope (for this phase):
- A full merge workflow that automatically produces superpositions for two lane heads.
  - This phase should make lane snaps "grabbable"; a follow-on can make them "mergeable".
- Organization-wide defaults, quotas, billing, and enterprise policy.

## Architecture Notes

Key split:
- `snap`: local checkpoint (low expectations)
- `sync`: durability + team-visible sharing (unpublished surface)
- `publish`: submission into gates/scopes (high-signal)

Lanes should feel like "team-visible branch heads":
- updating a lane head is cheap (just a pointer update) after object upload
- lanes are discoverable and fetchable by members

Retention:
- lane heads are GC roots
- server may enforce retention/quota policies per lane (keep last N, keep last X days, byte quota)

## Tasks

### A) Server data model: lane heads

- [x] Extend `Lane` to include per-member head pointers:
  - `snap_id`
  - `updated_at`
  - (optional) `client_id` for debugging
- [x] Persist lane heads in `repo.json` (serde defaults for backward compat).

### B) Server API: lanes + heads

- [x] Extend `GET /repos/:repo_id/lanes` to include lane membership + heads.
- [x] Add endpoint to update your head ("sync"):
  - `POST /repos/:repo_id/lanes/:lane_id/heads/me` with `{ snap_id }`
- [x] Add endpoint to read a specific head:
  - `GET /repos/:repo_id/lanes/:lane_id/heads/:user`

Access control:
- [x] Only lane members can read lane heads.
- [x] Only a user can update their own head.

### C) Client protocol: sync + fetch-by-lane

- [x] Implement `RemoteClient::sync_snap(...)`:
  - ensure snap + required objects uploaded (existing missing-object negotiation)
  - update lane head to the snap
- [x] Implement `RemoteClient::list_lanes()` with heads.
- [x] Implement `RemoteClient::fetch_lane_head(...)`:
  - resolve head snap id
  - fetch snap + manifest tree + required objects

Local state:
- [x] Record last-synced snap id per lane/user locally (for "unsynced" UI).

### D) CLI UX

- [x] `converge sync [--snap-id <id>] [--lane <lane>]` (defaults: latest/HEAD snap; default lane).
- [x] `converge lanes` (show members + heads).
- [x] `converge fetch --lane <lane> [--user <user>]` (fetch head snap and objects).

### E) TUI UX

- [x] Treat `publish` as a local command.
- [x] Add `sync` as a local root command.
- [x] Add a remote "Lanes" browser view:
  - [x] list lanes and per-member heads
  - [x] action: fetch a selected head
- [ ] Root hints:
  - [x] show "unsynced" indicator locally
  - [x] show lane discovery hints remotely

### F) Retention + GC

- [x] Update server GC roots to include lane heads (snaps + reachable objects).
- [x] Add lane retention policy (initial MVP): keep last N heads per user per lane.
- [x] Tests: GC does not delete lane head objects.

## Exit Criteria

- A user can `snap` locally and `sync` it to the server without publishing.
- Teammates can discover lane heads and `fetch --lane` another user's unpublished snap.
- Server GC retains lane head snaps and their reachable objects.
- `publish` remains the explicit act that submits work to gates/scopes.
