# 13 Rebuild Workspace and Crates

Status: active
Updated: 2026-07-23
Roadmap: `g02.002` Batch 2.4

Client and server are independent systems in one Cargo workspace, coupled only
through the shared model crate and the wire contract
([16-sync-protocol-and-chunking.md](./16-sync-protocol-and-chunking.md)).

## Workspace layout

```
crates/
  converge-model     # shared: object model, IDs, hashing, wire DTOs
  converge-client    # lib: workspace, local store, diff, resolve, remote client
  converge-cli       # bin `converge`: canonical verb surface over converge-client
  converge-tui       # bin: thin front-end over the CLI argv contract
  converge-server    # bin: control plane + data plane services
```

Rules:

- `converge-model` is the only crate both sides depend on. It holds the
  Merkle object model (`Manifest`, `ManifestEntryKind` incl. `Superposition`),
  ID and hash discipline (blake3, verify-on-read), and the wire DTOs that g01
  duplicated between client and server.
- `converge-server` never depends on `converge-client`, and vice versa.
- `converge-tui` depends on `converge-cli`'s command layer, not on
  `converge-client` internals — the TUI/CLI single-semantic-contract rule from
  the UX spec is enforced by crate structure.

## Salvage migration (first implementation roadmap)

From the current lib-only crate (per `docs/rebuild/003-salvage-inventory.md`):

- `src/model/` → `converge-model`
- `src/store/`, `src/diff/`, `src/resolve/`, `src/workspace/` →
  `converge-client` (store gains sharded object fanout; chunking modules are
  replaced, see doc 16)
- everything else is rebuilt, not migrated

## Next Task

Use this layout in the first rebuild implementation roadmap.
