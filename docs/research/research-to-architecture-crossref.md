# Research-to-Architecture Cross-Reference

Status: Active
Last updated: 2026-03-07
Purpose: Map translation memo findings to Convergence architecture docs, identify gaps, and track promotion status.

## Gap Analysis Results

Each memo below is read against the current architecture set and classified as `Aligned`, `Partially Aligned`, `Missing`, or `Prototype-Gated`.

### Memo 001: Snap Semantics -> `prototype first`

| Research Finding | Architecture Doc | Alignment | Gap Description |
| --- | --- | --- | --- |
| Snaps are captured automatically rather than created only by explicit user action | `01-concepts-and-object-model.md`, `07-client-workspace-architecture.md` | Partially Aligned | Architecture now describes automatic capture, but trigger strategy and UX still depend on `prototype-snap-capture.md`. |
| Snap message is optional and can be added later | `01-concepts-and-object-model.md`, `10-cli-and-tui.md` | Partially Aligned | The concept exists, but editing flow and reminder UX are still prototype work. |
| Build/test status is metadata, not a capture precondition | `01-concepts-and-object-model.md`, `03-operations-and-semantics.md` | Aligned | Architecture treats snap capture as reality capture and keeps buildability as evolving metadata. |
| Snaps stay local until explicit publish | `01-concepts-and-object-model.md`, `03-operations-and-semantics.md` | Aligned | Core object model and operation flow maintain local-first capture with explicit publish. |
| Storage overhead must be validated before commitment | `06-storage-and-data-model.md`, `prototype-snap-capture.md` | Prototype-Gated | Content-addressing direction exists, but measured storage behavior still needs prototype evidence. |

### Memo 002: Gate Policy Model -> `prototype first`

| Research Finding | Architecture Doc | Alignment | Gap Description |
| --- | --- | --- | --- |
| Gates are server-authoritative policy boundaries | `02-repo-gates-lanes-scopes.md`, `05-policy-model-and-phase-gates.md`, `08-server-authority-architecture.md` | Aligned | Current architecture already treats gate policy as server-enforced. |
| Promotion is explicit and policy-checked | `02-repo-gates-lanes-scopes.md`, `03-operations-and-semantics.md`, `05-policy-model-and-phase-gates.md` | Partially Aligned | Promotion semantics are present, but the operator and UX flow still depend on `prototype-gate-chain.md`. |
| Start with a simple linear gate chain before expanding to richer graphs | `05-policy-model-and-phase-gates.md`, `12-gate-graph-schema.md` | Prototype-Gated | Architecture supports broader gate graphs; the linear chain is the validation path, not yet proven in implementation. |
| Bundle production is the canonical output of a gate | `02-repo-gates-lanes-scopes.md`, `03-operations-and-semantics.md` | Aligned | Bundle semantics already anchor gate output. |
| Policy configuration and approval mechanics need usability validation | `05-policy-model-and-phase-gates.md`, `10-cli-and-tui.md` | Prototype-Gated | Policy structure exists, but authoring and promotion ergonomics still need prototype evidence. |

### Memo 003: Superposition as Data -> `promote to concept work`

| Research Finding | Architecture Doc | Alignment | Gap Description |
| --- | --- | --- | --- |
| Superpositions are first-class, addressable objects | `01-concepts-and-object-model.md`, `04-superpositions-and-resolution.md` | Aligned | Architecture now describes structured superposition identity and lifecycle. |
| Full provenance must be retained for each conflicting variant | `04-superpositions-and-resolution.md`, `06-storage-and-data-model.md` | Aligned | Research-driven provenance detail is integrated. |
| Resolutions should be recorded and reopenable | `04-superpositions-and-resolution.md` | Aligned | Resolution state and reopen semantics are now part of the architecture. |
| Bundles may exist in a partial state with unresolved superpositions | `01-concepts-and-object-model.md`, `03-operations-and-semantics.md`, `04-superpositions-and-resolution.md` | Aligned | The architecture accepts unresolved intermediate state rather than blocking all bundle creation. |
| Resolution UX and storage cost still need validation | `04-superpositions-and-resolution.md`, `10-cli-and-tui.md`, `06-storage-and-data-model.md` | Partially Aligned | The model is architecture-ready, but implementation UX and storage measurements are still open. |

## Critical Gaps

| Gap | Related Research | Architecture Area | Status |
| --- | --- | --- | --- |
| Snap trigger strategy and history UX are not yet validated | Memo 001 | `07-client-workspace-architecture.md`, `10-cli-and-tui.md` | Open, prototype-gated |
| Gate promotion flow and policy authoring ergonomics are not yet validated | Memo 002 | `05-policy-model-and-phase-gates.md`, `10-cli-and-tui.md`, `12-gate-graph-schema.md` | Open, prototype-gated |
| Superposition resolution UX and variant storage cost still need implementation evidence | Memo 003 | `04-superpositions-and-resolution.md`, `06-storage-and-data-model.md`, `10-cli-and-tui.md` | Open |

## Areas Already Aligned

| Finding | Research Source | Architecture Doc |
| --- | --- | --- |
| local-first capture with explicit publish | Memo 001 | `01-concepts-and-object-model.md`, `03-operations-and-semantics.md` |
| gate as server-authoritative policy boundary | Memo 002 | `02-repo-gates-lanes-scopes.md`, `05-policy-model-and-phase-gates.md`, `08-server-authority-architecture.md` |
| superposition as first-class conflict data | Memo 003 | `04-superpositions-and-resolution.md` |

## Prototype Dependency Ordering

### Tier 1: Immediate Validation

1. snap capture prototype — validates automatic capture semantics and storage behavior
2. gate chain prototype — validates policy enforcement and explicit promotion flow

### Tier 2: Follow-On Validation

1. superposition resolution UX — validates operator and contributor flows
2. storage benchmark for snap volume and superposition variants — refines data-model constraints

## Next Task

Keep this cross-reference current as prototype work either closes or sharpens the remaining research-to-architecture gaps.
