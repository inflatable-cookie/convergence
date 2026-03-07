# Prototype: Linear Gate Chain

**Status**: Design Complete — Ready for Implementation
**Research Basis**: [Translation Memo 002](/Users/betterthanclay/Dev/projects/convergence/docs/research/translation-memos/002-gate-policy-model.md)

## Goal

Build a prototype implementing a linear gate chain to validate:
1. Gate policy enforcement
2. Promotion UX
3. Bundle visibility
4. Policy configuration approach

## Prototype Scope

### In Scope

- Three-gate linear chain: Development → Integration → Release
- Simple policy checks (build status, approval count)
- Explicit promotion UX
- Bundle visualization

### Out of Scope

- Complex gate graphs (branching/merging)
- External CI integration
- Sophisticated policy language
- Automatic promotion

## Design

### Gate Structure

```rust
struct Gate {
    id: String,                  // "dev", "integration", "release"
    name: String,
    description: String,
    
    // Graph position
    upstream: Option<GateId>,    // Where bundles come from
    downstream: Option<GateId>,  // Where bundles can promote to
    
    // Policy
    policy: GatePolicy,
}

struct GatePolicy {
    required_build_status: BuildStatus,
    required_approvals: u32,
    auto_promote: bool,
}

enum BuildStatus {
    Unknown,
    Success,
    Failure,
}
```

### Linear Chain

```
[Development] → [Integration] → [Release]
     ↑                ↑              ↑
   accepts          accepts       accepts
   snaps          bundles from   bundles from
                  dev gate       integration
```

**Development Gate**:
- Accepts: Snaps from workspace publish
- Policy: Build passes
- Produces: Development bundles

**Integration Gate**:
- Accepts: Bundles from Development
- Policy: Build passes + 1 approval
- Produces: Integration bundles

**Release Gate**:
- Accepts: Bundles from Integration  
- Policy: Build passes + 2 approvals + no superpositions
- Produces: Release candidates

### Policy Configuration

```toml
# Server-side configuration
[[gate]]
id = "dev"
name = "Development"
downstream = "integration"

[[gate.policy]]
required_build_status = "success"
required_approvals = 0
auto_promote = false

[[gate]]
id = "integration"
name = "Integration"
upstream = "dev"
downstream = "release"

[[gate.policy]]
required_build_status = "success"
required_approvals = 1
auto_promote = false

[[gate]]
id = "release"
name = "Release"
upstream = "integration"

[[gate.policy]]
required_build_status = "success"
required_approvals = 2
auto_promote = false
```

### Promotion Flow

```
# User publishes snap to development
$ converge publish --to dev
Snap 01HQ... published to development gate
Gate processing... done
Bundle dev-01 created in development

# Check gate status
$ converge gates
Development
  Bundle: dev-01 (ready for promotion)
    Build: ✓ success
    Approvals: 0/0
    
Integration (upstream: Development)
  (no bundles)
  
Release (upstream: Integration)
  (no bundles)

# Promote to integration
$ converge promote dev-01 --to integration
Checking policy for integration gate...
✓ Build passes
✗ Need 1 approval (have 0)
Promotion blocked: insufficient approvals

# Add approval
$ converge approve dev-01
Approved bundle dev-01

# Try promotion again
$ converge promote dev-01 --to integration
Checking policy for integration gate...
✓ Build passes
✓ 1 approval (required: 1)
Promoting dev-01 to integration... done
Bundle int-01 created in integration

# View full chain
$ converge gates --tree
Development
  dev-01 → [promoted to integration]
Integration
  int-01 (ready for promotion)
    Build: ✓ success
    Approvals: 0/2
Release
  (no bundles)
```

### Bundle Structure

```rust
struct Bundle {
    id: Ulid,
    gate_id: GateId,
    
    // Source
    source_snap_ids: Vec<Ulid>,  // One or more snaps coalesced
    
    // Content
    root_manifest_id: ContentHash,
    superpositions: Vec<SuperpositionId>,
    
    // Status
    status: BundleStatus,
    build_status: BuildStatus,
    approvals: Vec<Approval>,
    
    // Provenance
    created_at: DateTime,
    promoted_from: Option<BundleId>,
}

enum BundleStatus {
    Pending,      // Being processed
    Complete,     // Ready for use/promotion
    Partial,      // Has unresolved superpositions
    Promoted,     // Has been promoted to next gate
}

struct Approval {
    identity: Identity,
    approved_at: DateTime,
    comment: Option<String>,
}
```

### Policy Enforcement

Policy is checked at promotion time:

```rust
fn check_promote_policy(bundle: &Bundle, target_gate: &Gate) -> Result<(), PolicyError> {
    // Check build status
    if bundle.build_status != target_gate.policy.required_build_status {
        return Err(PolicyError::BuildNotPassing);
    }
    
    // Check approvals
    if bundle.approvals.len() < target_gate.policy.required_approvals as usize {
        return Err(PolicyError::InsufficientApprovals {
            have: bundle.approvals.len(),
            need: target_gate.policy.required_approvals,
        });
    }
    
    // Check superpositions (for release gates)
    if target_gate.id == "release" && !bundle.superpositions.is_empty() {
        return Err(PolicyError::UnresolvedSuperpositions);
    }
    
    Ok(())
}
```

## Implementation Plan

### Phase 1: Basic Gates (2 days)

- [ ] Gate configuration loading
- [ ] Gate chain validation
- [ ] Bundle creation from snaps

### Phase 2: Promotion (2 days)

- [ ] Promotion command
- [ ] Policy checking
- [ ] Bundle movement between gates

### Phase 3: Approvals (1 day)

- [ ] Approval recording
- [ ] Approval requirements
- [ ] Approval audit log

### Phase 4: UX Polish (1-2 days)

- [ ] Gate visualization (`converge gates`)
- [ ] Bundle status display
- [ ] Promotion dry-run

### Phase 5: Testing (2-3 days)

- [ ] Policy enforcement tests
- [ ] Promotion workflow tests
- [ ] User study: gate understanding

## Success Criteria

1. **Clear promotion path** — Users understand how bundles flow
2. **Policy enforceable** — Checks run correctly, no bypass
3. **Visible status** — Easy to see what's ready to promote
4. **Attributable** — Know who promoted, who approved

## Test Scenarios

### Scenario 1: Happy Path

User publishes snap, it flows through gates to release.
- Expect: Smooth promotion with approvals
- Verify: Each gate produces bundle, policy enforced

### Scenario 2: Policy Block

Bundle fails policy check (e.g., no approval).
- Expect: Clear error message, promotion blocked
- Verify: Blocked bundle stays in current gate

### Scenario 3: Superposition Block

Bundle with unresolved superpositions tries to promote to release.
- Expect: Blocked at release gate
- Verify: Clear message about needing resolution

### Scenario 4: Multiple Bundles

Multiple bundles in development, promoting different ones.
- Expect: Independent promotion
- Verify: Each bundle tracked separately

## Metrics to Collect

During prototype testing:

1. **Promotion frequency** — How often do users promote?
2. **Policy block rate** — How often are promotions blocked?
3. **Approval latency** — Time from bundle to approval
4. **Gate dwell time** — How long bundles stay in each gate
5. **User confusion** — Do users understand gate semantics?

## Exit Criteria

Prototype is successful if:

- [ ] Users understand gate flow intuitively
- [ ] Policy enforcement feels fair, not arbitrary
- [ ] Promotion UX is smooth
- [ ] Gate visualization is helpful

## Next Steps After Prototype

1. If successful: Integrate into main codebase
2. If policy language too limited: Expand configuration options
3. If UX issues: Simplify or add visual gate graph
4. If successful: Proceed to integrate snap and gate prototypes

## Comparison to Existing Systems

| Feature | GitHub Protection | Perforce Streams | Convergence Gates (Proto) |
|---------|-------------------|------------------|---------------------------|
| Structure | Per-branch | Hierarchical tree | Linear chain |
| Promotion | Merge | Copy up/upmerge | Explicit promote |
| Policy | Rules | Structure-enforced | Configurable checks |
| Visibility | PR UI | Stream graph | Gate tree |
| Flexibility | Medium | Low | Medium |

## References

- [Translation Memo 002: Gate Policy Model](/Users/betterthanclay/Dev/projects/convergence/docs/research/translation-memos/002-gate-policy-model.md)
- [Track 2: Gate-Based Workflows](/Users/betterthanclay/Dev/projects/convergence/docs/research/value-tracks/gate-based-workflows.md)
- [02-repo-gates-lanes-scopes.md](./02-repo-gates-lanes-scopes.md)
