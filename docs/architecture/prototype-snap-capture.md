# Prototype: Automatic Snap Capture

**Status**: Design Complete — Ready for Implementation
**Research Basis**: [Translation Memo 001](/Users/betterthanclay/Dev/projects/convergence/docs/research/translation-memos/001-snap-semantics.md)

## Goal

Build a prototype implementing automatic snap capture to validate:
1. Appropriate capture frequency
2. UX for continuous vs. explicit capture
3. Storage overhead at scale
4. User mental model for snap vs. publish

## Prototype Scope

### In Scope

- Automatic snap capture (time-based and change-based)
- Snap history navigation
- Optional message attachment
- Build status tracking
- Storage metrics

### Out of Scope

- Network sync (local only)
- Complex policies
- Integration with gates/bundles
- Advanced pruning

## Design

### Snap Structure

```rust
struct Snap {
    id: Ulid,                    // Time-sortable
    workspace_id: Uuid,
    
    // Content
    root_manifest_id: ContentHash,
    
    // Metadata
    message: Option<String>,     // Optional at capture time
    created_at: DateTime,
    
    // Automatic metadata
    trigger: SnapTrigger,        // Why captured
    build_status: BuildStatus,
}

enum SnapTrigger {
    Automatic { reason: AutoTrigger },
    Explicit,                     // User manually triggered
}

enum AutoTrigger {
    TimeInterval,               // N minutes elapsed
    ChangeDetected,             // Files changed + idle
    BuildCompleted,             // Build finished
}

enum BuildStatus {
    Unknown,
    Pending,
    Success,
    Failure { error: Option<String> },
}
```

### Capture Triggers

**Time-based**: Every N minutes (default: 5)
- Simple, predictable
- May capture unchanged state

**Change-based**: After files change and idle period (default: 30 seconds)
- Captures meaningful changes
- May miss rapid changes

**Hybrid** (recommended): Time OR change-based, whichever comes first
- Captures at least every N minutes
- Also captures after meaningful idle

### Configuration

```toml
# .converge/config.toml
[snap]
enabled = true
trigger = "hybrid"  # "time", "change", "hybrid", "explicit"

[snap.time_based]
interval_minutes = 5

[snap.change_based]
idle_seconds = 30

[snap.storage]
max_snaps_per_workspace = 100
prune_after_days = 7
```

### UX Flow

```
# User works normally...
# (snaps captured automatically in background)

# Check current state
$ converge status
Workspace: my-project
Current snap: 01HQ... (2 minutes ago)
  Message: (none)
  Build: succeeded
  Changes since: 3 files modified

# View snap history
$ converge snaps --last 10
01HQ...  2 min ago   (no message)     [build: ok]
01HP...  5 min ago   "WIP auth"         [build: failed]
01HO...  12 min ago  "Add login form"   [build: ok]
...

# Add message to current snap
$ converge snap --message "Add password validation"
Message added to snap 01HQ...

# Go back to previous snap
$ converge restore 01HO...
Restored workspace to snap 01HO...
```

### Storage Optimization

1. **Content-addressing** — Deduplicated blob storage
2. **Delta compression** — Similar to Git packfiles
3. **Pruning** — Automatic removal of old snaps
   - Keep last N snaps
   - Keep snaps with messages
   - Keep snaps before/after build status change

### Build Status Tracking

Prototype includes optional build status:

```bash
# Trigger build after snap
$ converge snap --build
Capturing snap... done (01HQ...)
Building... done
Snap updated: build_status = Success

# Or continuous build mode
$ converge config set snap.build_on_capture true
```

## Implementation Plan

### Phase 1: Basic Capture (1-2 days)

- [ ] Time-based capture daemon
- [ ] Snap creation from workspace state
- [ ] Snap listing (`converge snaps`)

### Phase 2: UX Polish (1-2 days)

- [ ] Change-based capture
- [ ] Message attachment
- [ ] Restore from snap

### Phase 3: Metrics (1 day)

- [ ] Storage usage tracking
- [ ] Capture frequency metrics
- [ ] Pruning implementation

### Phase 4: Testing (2-3 days)

- [ ] User study: capture frequency preferences
- [ ] Storage benchmark: various project sizes
- [ ] Build status integration test

## Success Criteria

1. **No lost work** — User can always recover recent state
2. **Transparent** — Capture doesn't interrupt workflow
3. **Discoverable** — User understands snap concept quickly
4. **Efficient** — Storage overhead < 20% of working directory

## Test Scenarios

### Scenario 1: Rapid Editing

User rapidly edits file for 10 minutes.
- Expect: Multiple snaps captured
- Verify: Can restore to intermediate states

### Scenario 2: Long Running Work

User works for 2 hours, periodic saves.
- Expect: Regular snaps throughout
- Verify: History shows progression

### Scenario 3: Build Failure Recovery

User makes changes, build fails, wants to go back.
- Expect: Snap before failure available
- Verify: Can restore and compare

## Metrics to Collect

During prototype testing:

1. **Capture frequency** — How often do snaps occur?
2. **Storage growth** — GB per day of active development
3. **Restore usage** — How often do users restore?
4. **Message addition** — Do users add messages? When?
5. **User sentiment** — Does automatic capture feel helpful or intrusive?

## Exit Criteria

Prototype is successful if:

- [ ] Users prefer automatic over explicit capture
- [ ] Storage overhead is acceptable (< 20%)
- [ ] Users understand snap vs. publish distinction
- [ ] No complaints about capture frequency

## Next Steps After Prototype

1. If successful: Integrate into main codebase
2. If storage concerns: Implement aggressive pruning
3. If UX issues: Adjust capture triggers
4. If successful: Proceed to gate prototype

## References

- [Translation Memo 001: Snap Semantics](/Users/betterthanclay/Dev/projects/convergence/docs/research/translation-memos/001-snap-semantics.md)
- [Track 1: Continuous Capture vs. Explicit Commit](/Users/betterthanclay/Dev/projects/convergence/docs/research/value-tracks/continuous-capture-vs-explicit-commit.md)
- [01-concepts-and-object-model.md](./01-concepts-and-object-model.md)
