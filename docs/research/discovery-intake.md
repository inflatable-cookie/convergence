# Discovery Intake and Frontier Triage

Purpose: define how low-authority secondary channels feed into the research program without polluting the primary-source corpus.

## Why This Exists

The research program's source hierarchy requires official docs, papers, and source trees before secondary commentary. But secondary channels surface signals faster than primary-source indexing — new VCS releases, research papers, production insights, and tool announcements often appear on Hacker News, Lobste.rs, or Mastodon days before formal publication. Without an intake process, the program either misses timely signals or absorbs unvetted claims into the corpus.

## Discovery Channel Registry

### Tier A — Curated Aggregators (weekly check cadence)

These channels have editorial judgment and consistent track records of linking to primary sources:

| Channel | Signal Type | Timeliness | Primary Failure Mode |
| --- | --- | --- | --- |
| Hacker News (VCS tags) | tool releases, paper links, practitioner discussion | same-day | novelty bias; shallow technical depth in comments |
| Lobste.rs (VCS, git, mercurial tags) | practitioner links, release notes | 0-3 days | smaller community; may miss enterprise signals |
| This Week in Rust (if VCS-related) | Rust ecosystem tools | weekly | language-specific bias |
| ACM Digital Library alerts | formal papers | weeks | academic-only; misses industry practice |

### Tier B — Production Testing and Analysis (event-driven check)

These channels provide empirical evidence about shipped implementations:

| Channel | Signal Type | Timeliness | Primary Failure Mode |
| --- | --- | --- | --- |
| Game Developer / Gamasutra | postmortems, workflow articles | weeks-months | promotional framing; limited technical depth |
| GDC Vault (free tier) | production talks | post-conference | vendor-heavy; requires primary source tracing |
| Git Merge conference talks | Git ecosystem evolution | annual | Git-centric; may miss alternatives |

### Tier C — Technical Explainers (as-needed reference)

These channels provide implementation-level education, not discovery:

| Channel | Signal Type | Primary Failure Mode |
| --- | --- | --- |
| Julia Evans (wizardzines) | systems concepts explained | educational framing; not comparative |
| Various VCS documentation | official tutorials and guides | vendor-biased toward own model |

### Tier D — Community Forums and Chat (ephemeral signal, never cite directly)

These channels surface practitioner reactions but are ephemeral:

| Channel | Signal Type | Primary Failure Mode |
| --- | --- | --- |
| r/git, r/mercurial, r/programming | user questions, workflow discussions | uncurated; beginner-heavy |
| Discord/Matrix VCS channels | implementation help, tool comparisons | ephemeral; not search-indexed |
| Twitter/X tech community | hot takes, release announcements | extreme noise; vendor marketing |

## Triage Rules

Every signal from a secondary channel must be triaged before it can enter the research corpus. Triage produces exactly one outcome per signal.

### Triage Outcomes

| Outcome | Meaning | What Happens Next |
| --- | --- | --- |
| `research now` | primary source exists, Convergence-relevant, strong enough to enter a value track or memo | trace to primary source, add to relevant source map and value track |
| `lead only` | interesting signal but primary source is missing, incomplete, or unverified | record in the triage log with the claim and the missing primary source; do not add to corpus |
| `watch` | credible primary source exists but the technique is too early, too niche, or too uncertain to act on | record in the triage log with the primary source and a review trigger condition |
| `reject` | not Convergence-relevant, or the claim does not survive primary-source tracing | record in the triage log with the reason for rejection; do not add to corpus |

### Triage Decision Tree

1. **Does a primary source exist?** (paper, official docs, talk, source tree, first-party writeup)
   - No → `lead only` (record what the claim is and what source is missing)
   - Yes → continue

2. **Is the technique Convergence-relevant?** (does it address a problem in gate workflows, superpositions, binary handling, or convergence semantics?)
   - No → `reject`
   - Yes → continue

3. **Is the primary source strong enough?** (tier 1-2 in the source hierarchy)
   - No → `lead only` (record the weak source and what would strengthen it)
   - Yes → continue

4. **Is the technique ready for Convergence action?** (shipping in production, has multiple implementations, or has production evidence)
   - No → `watch` (record the technique, primary source, and what would make it ready)
   - Yes → `research now`

### Triage Quality Rules

- Never add a `lead only` or `watch` item directly to a value track or translation memo. These stay in the triage log until they are promoted through re-triage.
- Never cite a secondary channel as a source in the research corpus. Always trace to the primary source.
- When a `watch` item's review trigger fires (e.g., a second system implements the technique, a paper is published), re-triage it.
- When a `lead only` item's missing primary source appears, re-triage it.
- Triage is not permanent — items can move between outcomes as evidence changes.

## Triage Log Format

Each triage entry records:

```
## [Signal Title]

- Source channel: [which secondary channel surfaced this]
- Date triaged: [YYYY-MM-DD]
- Claim: [what the signal claims, in one sentence]
- Primary source: [link to primary source if found, or "missing" with what would constitute one]
- Convergence relevance: [which value track(s) this relates to, or "none"]
- Outcome: [research now | lead only | watch | reject]
- Reason: [one sentence explaining the triage decision]
- Review trigger: [for watch items only — what event would cause re-triage]
```

## Check Cadence

| Channel Tier | Check Frequency | Who |
| --- | --- | --- |
| Tier A (curated aggregators) | weekly | research session |
| Tier B (production testing) | event-driven (releases, conferences) | research session |
| Tier C (technical explainers) | as-needed when a topic requires background | research session |
| Tier D (community forums) | never systematically; only when a specific signal is reported | research session |

## Integration with Research Corpus

- `research now` items get added to the relevant source map and value track following normal research batch procedures.
- `lead only` items stay in the triage log until their primary source appears.
- `watch` items stay in the triage log until their review trigger fires.
- `reject` items stay in the triage log permanently as a record of what was considered and why it was excluded.
- The triage log is not a value track — it is a staging area. Nothing in the triage log is citable by concept work or roadmaps.

## Next Task

Run the initial triage pass on current secondary signals to populate the triage log and validate the intake process. Priority signals to capture:
- Sapling (Meta's Git-compatible VCS)
- Jujutsu (Google's Rust-based VCS)
- Any announced but unreleased "Git killers"
