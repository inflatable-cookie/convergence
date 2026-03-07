# Research-to-Implementation Playbook

Status: Active
Last updated: 2026-03-07
Purpose: Ensure Convergence research findings actively inform implementation instead of remaining isolated in the research corpus.

## The Problem

Research only matters if it changes build decisions.
This playbook keeps architecture, prototypes, implementation, validation, and review tied back to the research corpus.

## Workflow: Research-Aware Delivery

### Phase 1: Discovery

1. Identify the architecture doc or prototype the task belongs to.
2. Check `master-index.md` to find the relevant memo, value track, dossier, and validation work.
3. Read the translation memo first.
4. Confirm whether the memo outcome is `prototype first` or already integrated into architecture.

### Phase 2: Decision

Before writing implementation code, record:
- which research artifacts were consulted
- which recommendations are being followed directly
- which recommendations are being deferred or rejected
- which open questions still require prototype data

Use `templates/implementation-decision-record.md` when the decision should remain durable.

### Phase 3: Implementation

- Reference the research basis in code comments when behavior is intentionally derived from a memo or dossier pattern.
- If implementation uncovers a missing research area, add it to `gaps-found-during-implementation.md`.
- If the code needs to deviate from the research, document the reason explicitly instead of silently drifting.

### Phase 4: Validation

- Derive tests from research-backed behavior claims where practical.
- Treat prototype-gated recommendations as unsettled until the prototype produces evidence.
- Record the validation performed in the roadmap batch log.

### Phase 5: Review

Reviewers should check:
- the task consulted the right memo and supporting research
- deviations are documented and justified
- missing research was captured as a gap
- prototype-gated recommendations were not treated as settled without evidence

## Convergence-Specific Starting Points

| If you are building... | Start with... |
| --- | --- |
| snap capture, workspace history, local state | Memo 001 + `prototype-snap-capture.md` |
| gates, promotion, bundle policy, approvals | Memo 002 + `prototype-gate-chain.md` |
| superposition storage, resolution, reopen flows | Memo 003 + `04-superpositions-and-resolution.md` |
| storage or manifest behavior | Memo 001 / 003 + `06-storage-and-data-model.md` |
| CLI/TUI flows for snap, promote, or resolve | the relevant memo + `10-cli-and-tui.md` |

## Lightweight Checklist

- [ ] I checked `master-index.md`.
- [ ] I read the relevant translation memo.
- [ ] I know whether this area is architecture-ready or still prototype-gated.
- [ ] I documented major decisions or deviations if needed.
- [ ] I captured any missing research in `gaps-found-during-implementation.md`.

## When Research Is Missing

1. Do a quick targeted scan if the missing answer is likely to be cheap to find.
2. Record the gap when the answer still is not clear.
3. Make the provisional decision explicit.
4. Queue deeper research or prototype work if the risk is material.

## Next Task

Use this playbook for the first implementation batch that touches snap capture, gate promotion, or superposition resolution, then trim any steps the team never actually uses.
