# Batch 23.4 — Dashboard Recommendations Complete

Date: 2026-07-25
Roadmap: `g02.023`
Card: `085-dashboard-recommendations`

## The Rule

Spec 002 §4.7 deferred this on the grounds that it needed a ranking rule
rather than "what the inbox said". The rule is **what blocks other
people, first**:

1. a superposed bundle — nothing downstream moves until it is resolved
2. an approval — one publisher is waiting on you specifically
3. lane work you could pull — available, but blocking nobody
4. a publication — news

It lives on `ActionKind` and is applied by sorting inside
`converge_cli::inbox_actions`. That placement is the point: the Inbox
view and the Root dashboard read one order **by construction**. Ranking
in the TUI would have been a second rule waiting to drift from the
first, which is what the deferral was worried about.

The existing inbox test asserted the old, unranked order. It was
rewritten around the rule rather than adapted to the new output — a test
that says "resolve comes first because it blocks the gate for everyone"
survives a refactor that one asserting index 0 does not.

## Owner Labels Needed Real Data

The first pass read `published_by` off the inbox bundle. No such field
exists. The test passed because it invented one — a test asserting
against data the server never produces, which is the failure mode of
writing the test and the code from the same wrong assumption.

`InboxBundle` now carries `contributors`, built from the publications the
bundle consumed. That is exactly "who is waiting on this bundle".

Bounded by `INBOX_CONTRIBUTOR_SCAN`: a coalesced window can hold a
hundred publications, and reading all of them per gate per inbox call
would make a cosmetic label the most expensive thing in the response.
The cap is named on the wire type, so a client knows the list is partial
rather than assuming it is complete.

## Enter Does The Top Thing

A dashboard that ranks work and then makes `Enter` do something unrelated
has not helped. Root's primary action is now the top-ranked runnable
recommendation.

Uncaptured local work still outranks it. It is the only thing on that
screen that can be lost.

A group with more than one runnable member offers no command at all: the
dashboard reports, it does not pick one of five bundles for you.

## The 23.1 Finding Came Back

The first version spelled the command out in each row — `→ converge
resolve list` followed by 64 hex characters — and used the same string as
the primary-action label. Driving it showed the row cut off at the right
edge and the hint bar pushed so far right that `Tab` and `q` fell off the
screen. Exactly the defect batch 23.1 found in History and the Inbox,
reintroduced in a new place three batches later.

Rows now name the view. The primary action is labelled by kind
("resolve superpositions"). A render test fails if a full bundle id
reaches the dashboard at all, so the next reintroduction is caught in CI
rather than by eye.

The pasteable command did not go away — the Inbox is where a row is one
command you can copy, which has been the contract since batch 16.1. The
dashboard summarises; the Inbox acts.

## Loading Without Navigating

Root ranks from the inbox report, so the inbox has to load before anyone
asks for it. The existing `Intent::Inbox` pushes the Inbox view when its
result arrives, which would have yanked the user out of whatever they
were doing every time the dashboard refreshed. A data-only intent fills
the state and navigates nowhere.

## Validation

- `cargo fmt --all`, `cargo clippy --all-targets -D warnings`: clean
- `cargo nextest run --workspace`: 272 passed, 4 skipped
- driven against a real server with a real superposition and two people

## Next Task

Batch card 23.5 (superposition detail): the 65/35 split, which batch
23.1 recorded as a decision-correctness problem rather than polish.
