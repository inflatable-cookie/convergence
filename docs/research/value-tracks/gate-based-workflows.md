# Track 2: Gate-Based Workflows and Phased Convergence

## Problem Statement

How do code changes progress from development to production? Different systems enforce quality checkpoints differently:

- **Git branches** — Named lines of development, no inherent policy
- **GitHub/GitLab protection** — Rules on specific branches
- **Perforce streams** — Hierarchical branching with enforced merge paths
- **CI/CD gating** — External systems check code before merge

Convergence introduces `gate` as a policy boundary. How does this relate to:
- Branch protection?
- Required checks?
- Promotion workflows?
- Release readiness?

## Cross-System Comparison

### Git: Branches as References

Git branches are **lightweight references**:

```
main      → commit abc123
feature-x → commit def456
```

Branches have **no inherent policy** — they're just pointers. Policy is bolted on by:

1. **Server-side hooks** (Gitolite, etc.)
2. **Platform features** (GitHub, GitLab)
3. **CI/CD pipelines**

**Strengths**:
- Extremely flexible
- Any workflow possible

**Pain Points**:
- No built-in gate concept
- Policy enforcement inconsistent
- "Bypass" is always possible with enough permissions
- Branch sprawl at scale

### GitHub: Branch Protection Rules

GitHub adds **branch protection** to Git:

```yaml
# Branch protection settings
- Require pull request reviews
- Require status checks (CI)
- Require conversation resolution
- Include administrators
```

**Protected branches** enforce:
- No direct push (must go through PR)
- Required approvals
- Required CI checks passing
- Up-to-date with base branch

**Strengths**:
- Clear policy enforcement
- Integrates with review workflow

**Pain Points**:
- Binary (protected/not protected)
- No intermediate gates
- Policy is per-branch, not per-change
- Bypass possible for admins

### Perforce Streams: Structured Promotion

Perforce **streams** enforce promotion paths:

```
mainline (main)
├── development
│   ├── feature-1
│   └── feature-2
└── release/1.0
```

**Stream types**:
- `mainline` — Root of hierarchy
- `development` — Flows to/from parent
- `release` — Stabilization, flows from mainline
- `task` — Lightweight, flows to parent
- `virtual` — Filter/view of other streams

**Merge paths are enforced**:
- Can only merge from parent or children
- Cannot merge arbitrarily between siblings

**Flow rules**:
- **Merge down** — Bring changes down (resolve conflicts early)
- **Copy up** — Promote stable changes up

**Strengths**:
- Enforced promotion paths
- Clear integration order
- Hierarchical visibility

**Pain Points**:
- Rigid structure
- Reworks when structure doesn't match team reality
- All or nothing promotion

### GitLab: Merge Request Approvals

GitLab adds **approval rules** to merge requests:

```yaml
# Approval rules
- Code owners must approve
- Security team must approve for security files
- CI must pass
- MR must be up-to-date
```

**Multi-level gates**:
- Required approvals (count + specific people)
- Code owner approvals
- External approval rules (via API)

**Strengths**:
- Flexible approval workflows
- Can require multiple approvals

**Pain Points**:
- Still branch-centric
- Late feedback (after code written)
- No intermediate "staging" states

### CI/CD Gating: External Enforcement

Modern CI/CD provides **quality gates**:

```yaml
# GitHub Actions example
- name: Test
  run: npm test
- name: Security scan
  run: security-scan
- name: Deploy to staging
  if: github.ref == 'refs/heads/main'
```

**Gate types**:
- **Build gates** — Compiles? Tests pass?
- **Security gates** — No vulnerabilities?
- **Performance gates** — Benchmarks within threshold?
- **Manual gates** — Human approval required?

**Strengths**:
- Arbitrary complexity
- Can integrate any check

**Pain Points**:
- External to VCS
- Late feedback (after push)
- Configuration drift
- CI-specific (not portable)

## Pattern Analysis

### Gate Enforcement Locations

| Location | Examples | Strength | Weakness |
|----------|----------|----------|----------|
| Server hooks | Gitolite | Low overhead | Hard to configure |
| Platform rules | GitHub protection | User-friendly | Platform lock-in |
| Stream hierarchy | Perforce | Enforced structure | Rigid |
| CI/CD | GitHub Actions | Flexible | Late feedback |

### Promotion Models

| Model | Systems | Flexibility | Enforcement |
|-------|---------|-------------|-------------|
| Free-form branches | Git | High | None |
| Protected branches | GitHub/GitLab | Medium | Server-side |
| Hierarchical streams | Perforce | Low | Structure-enforced |
| Pipeline stages | CI/CD | High | External |

### Repeat Failures

1. **Late feedback** — CI catches issues after code is "done"
2. **Bypass risk** — Admins can bypass protection
3. **Binary pass/fail** — No "needs work but promising" state
4. **Rigidity** — Perforce streams don't adapt to changing team structures
5. **Tool fragmentation** — Policy split across VCS, platform, CI

## Frontier Work

### Emerging Approaches

- **Stacked PRs** — Phabricator, Sapling, Jujutsu: Review units smaller than branches
- **Merge queues** — GitHub, GitLab: Batch and verify merges
- **Policy as code** — Open Policy Agent (OPA) for VCS rules
- **Continuous verification** — Checks run continuously, not just at PR time

### Unexplored Territory

- **Probabilistic gates** — "80% confident this is safe"
- **Personal gates** — Different policies per developer experience
- **Temporal gates** — Different policies at different times (release freeze)

## Convergence Implications

### Core Insight

Convergence's `gate` is different from existing approaches:

- **Not a branch** — Gates are policy boundaries, not lines of development
- **Not just protection** — Gates produce `bundle` outputs
- **Not external CI** — Gates are part of the system
- **Hierarchical** — Like Perforce streams, but more flexible

### Recommended Direction

Based on cross-system analysis:

1. **Gates are server-authoritative** — Policy lives on server, enforced there
2. **Gates produce bundles** — Output is a coalesced, named artifact
3. **Multiple gates in series** — Development → Integration → Staging → Release
4. **Gate policy is configurable** — Not hardcoded like Perforce stream types
5. **Promotion requires passing policy** — Explicit "promote" operation

### Comparison to Existing Systems

| Feature | Git Branch | GitHub Protection | Perforce Stream | Convergence Gate |
|---------|------------|-------------------|-----------------|------------------|
| Policy | None | Server-side rules | Structure-enforced | Configurable, enforced |
| Output | N/A | N/A | N/A | Bundle artifact |
| Promotion | Merge | Merge | Copy up | Explicit promote |
| Visibility | Full history | Full history | Stream view | Gate scope |
| Flexibility | Maximum | Medium | Low | Medium-High |

### Tradeoffs Accepted

- **Server authority required** — Gates don't work offline
  - Mitigation: Queue promotions for when online
  
- **Complexity** — Gate graphs can become complex
  - Mitigation: Visual tools, sensible defaults

- **Learning curve** — New concept (not branches)
  - Mitigation: Map to familiar concepts initially

## Open Questions

1. Can gates form arbitrary DAGs or just linear/parallel?
2. How are gate policies defined? (DSL? Code? Config?)
3. What happens if policy changes mid-flight?
4. Can bundles be "unpromoted" or rolled back?
5. How do lanes interact with gates?

## Prototype Needs

Before finalizing gate semantics:

1. **Policy language experiment** — Test declarative vs. imperative policy
2. **Gate graph UX** — How to visualize and navigate gates?
3. **Promotion workflow** — What does "promote" feel like?

## References

- Dossiers: Git (branches), Perforce (streams), Plastic (branch explorer)
- Source map: 002-gate-policy-and-promotion (pending)
- Related tracks: Track 9 (permissions), Track 10 (CI/CD integration)
