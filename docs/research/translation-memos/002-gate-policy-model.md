# Translation Memo 002: Gate Policy Model

## Problem Statement

Convergence introduces `gate` as a policy boundary. How does gate policy work?

Research question: Where does policy live? How is it enforced? What happens at promotion?

## External Evidence

### Research Findings (Track 2)

Four approaches compared:

| Approach | Examples | Flexibility | Enforcement |
|----------|----------|-------------|-------------|
| Branch protection | GitHub, GitLab | Medium | Server-side rules |
| Hierarchical streams | Perforce | Low | Structure-enforced |
| CI/CD gating | GitHub Actions | High | External |
| Server hooks | Gitolite | Low | Script-based |

Key finding: **Policy enforcement location determines flexibility and timing**.

### Key Insights from Research

1. **Late feedback is costly** — CI catches issues after code is "done"
2. **Rigidity causes workarounds** — Perforce stream bypasses common
3. **External policy fragments** — CI, platform, hooks don't integrate
4. **Promotion paths need structure** — Perforce streams show value of defined paths

## Cross-System Comparison

### Policy Enforcement Models

| Model | Enforcement | Config | Pros | Cons |
|-------|-------------|--------|------|------|
| Platform rules | Server | UI/Config | User-friendly | Platform lock-in |
| Stream structure | Server | Structure | Enforced paths | Rigid |
| CI/CD | External | YAML | Flexible | Late feedback |
| Hooks | Server | Scripts | Arbitrary | Complex |

### Promotion Patterns

| System | Promotion Unit | Mechanism | Visibility |
|--------|---------------|-----------|------------|
| Git | Commit/branch | Push | Immediate |
| GitHub | PR merge | UI + rules | Immediate |
| Perforce | Changelist | Submit | Immediate |
| Streams | Stream merge | Copy up | Immediate |
| **Convergence** | **Bundle** | **Promote** | **Phase-aware** |

## Convergence Implications

### Recommended Gate Model

```
gate: {
  id: string,
  name: string,
  
  # Policy
  policy: {
    required_checks: [check_id],
    required_approvals: { count: number, from: [role] },
    auto_promote: boolean,
  },
  
  # Graph position
  upstream_gates: [gate_id],     # Where bundles come from
  downstream_gates: [gate_id],   # Where bundles can promote to
  
  # Output
  produces: "bundle",
}
```

### Key Decisions

1. **Gates are server-authoritative** — Policy lives on and is enforced by server
2. **Gates form a DAG** — Can have multiple upstream/downstream gates
3. **Promotion is explicit** — User initiates `promote`, server checks policy
4. **Policy is configurable** — Not hardcoded like Perforce stream types
5. **Bundles are produced** — Gates consume publications, produce bundles

### Gate Types (Initial)

```
# Development gate
- Accepts: snaps from workspace publish
- Policy: build passes
- Produces: development bundles

# Integration gate
- Accepts: bundles from development
- Policy: build + tests pass, 1 approval
- Produces: integration bundles

# Release gate
- Accepts: bundles from integration
- Policy: all checks pass, 2 approvals, security scan
- Produces: release candidates
```

### Promotion Flow

```
workspace → publish → [gate: development] → bundle
                                           ↓
                                  promote → [gate: integration] → bundle
                                                                  ↓
                                                         promote → [gate: release] → release
```

### Policy Enforcement Points

| Point | Action | Enforced By |
|-------|--------|-------------|
| Publish | Snap enters gate intake | Server (permissions) |
| Gate evaluation | Coalesce snaps into bundle | Server (policy) |
| Promote | Bundle advances to next gate | Server (policy check) |
| Release | Bundle becomes release | Server (release policy) |

### UX Implications

```bash
# See gate status
$ converge gates
mainline
├── development (3 bundles)
├── integration (1 bundle) ← 2 pending promotion
└── release (0 bundles)

# Promote a bundle
$ converge promote bundle-abc --to integration
Checking policy for integration gate...
✓ Build passes
✓ 1 approval (required: 1)
Promoting bundle-abc to integration...
Done. Bundle bundle-def created in integration.
```

## Tradeoffs Accepted

### Server Authority Required

**Concern**: Gates don't work offline

**Acceptance**: Correct. Gates are organizational policy, requiring server.

**Mitigation**:
- Queue promotions when offline
- Local policy simulation (best effort)
- Clear offline indicators

### Complexity

**Concern**: Gate graphs can become complex

**Mitigation**:
- Sensible defaults (linear gates)
- Visual gate graph explorer
- Lane-based scoping (different lanes, different gates)

### Learning Curve

**Concern**: Gates are new concept

**Mitigation**:
- Map to familiar concepts ("like protected branches")
- Progressive disclosure (simple by default)
- Good documentation

## Open Questions

1. **Policy language** — Declarative config? Code? Rules engine?
2. **Check execution** — In-process? Webhook? Sandboxed?
3. **Policy inheritance** — Do child gates inherit parent policy?
4. **Emergency bypass** — Is there always an escape hatch?
5. **Gate monitoring** — Metrics on gate health, throughput?

## Prototype Validation Needed

Before final adoption:

1. **Policy DSL experiment** — Test configuration approaches
2. **Gate graph UX** — Visual design for gate navigation
3. **Promotion workflow** — Test with real teams

## Recommended Next Step

**Outcome**: `prototype first`

Create a prototype implementing:
1. Linear gate chain (dev → integration → release)
2. Simple policy checks (build status, approval count)
3. Explicit promotion UX
4. Bundle visualization

Test gate workflow with pilot teams before complex policy features.

## Relationship to Architecture

### Updates to Architecture Docs

Update `docs/architecture/02-repo-gates-lanes-scopes.md`:
- Add gate policy structure
- Document promotion mechanics
- Define gate graph invariants

Update `docs/architecture/05-policy-model-and-phase-gates.md`:
- Reference this memo
- Document policy enforcement points

## References

- Value Track: [Track 2: Gate-Based Workflows](../value-tracks/gate-based-workflows.md)
- Dossiers: Perforce (streams), GitHub (branch protection)
- Architecture: `docs/architecture/02-repo-gates-lanes-scopes.md`
