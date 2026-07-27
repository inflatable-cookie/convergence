# Post-Fix Re-Drive

Batch 22.4 closed with 34 findings, `g02.026` added gate administration,
and between them roughly a dozen fixes landed. Those fixes are the
least-driven code in the tree, and the repo is now a shape it has never
been in: a staged graph, with work that has been promoted and released
through it.

So: drive it again, as a user, over the surface the changes touched.

## 1. The inbox had no idea a stage existed (fixed)

Publishing into a staged graph reported `ready to promote`. The inbox —
whose whole job is "what needs your attention" — listed the publication
as news and said nothing about the bundle.

The recommendation logic ended at two states:

```rust
BundleStatus::Ready { promotable: false } => "resolve",
BundleStatus::Ready { promotable: true } if approvals < required => "approve",
_ => continue,
```

Ready and approved fell through. Under a single gate that is right —
there is nowhere to promote to, so nothing is waiting. A staged graph
makes it the most common actionable state there is, and the action queue
dropped it.

Now recommended as `promote`, ranked below `approve` (which unblocks it)
and above lane activity (because until it moves, nothing downstream sees
the work). The onward gate is named when exactly one accepts the bundle,
so the row is a runnable command; a fan-out gets a label and no guess,
which is batch 23.4's rule applied to a new verb.

## 2. The same approval bug, in a fourth place (fixed)

Batch 26.4 fixed `promote` reading `required_approvals` off the gate that
*produced* a bundle rather than the gate it is leaving. The inbox had its
own copy of that mistake, and it survived a batch longer.

The consequence was worse than a wrong number, because the inbox is
where people get their commands:

```
bundle e18906c7ff67 @ intake -> promote (0/0)
    run: converge promote e18906c7ff67… --to release
```

`review` requires one approval. The queue recommended a promotion, and
the server refused it for want of an approval the queue had not asked
for. Now `(0/1) -> approve`, then `(1/1) -> promote`.

Worth stating plainly: this is the third time this session that fixing a
thing in one place left a copy live elsewhere — after the ignore rules
(three copies, batch 22.4 finding 9) and bundle-id prefixes (server
fixed, local snaps missed, finding 26). The lesson is not "look harder";
it is that a rule with more than one implementation will drift, and the
fix is to have one.

## 3. A promoted bundle reported the gate it had left (fixed)

`InboxBundle.gate_id` is where a bundle was *built*, and never changes.
The row rendered it as the bundle's location, so work two stages along
still read `@ intake`. Rows now show where the work has reached, falling
back to the producing gate when it has not moved.

## 4. The TUI showed you the state before your change (fixed)

Added a gate through the wizard, was returned to the gate screen, and
saw the graph as it had been. The command had worked — the CLI listed
the new gate — but the screen was the last thing to know.

`Intent::Command` refreshes status and history, which is what Root and
History read, and nothing else. Every list view — gates, bundles,
releases, lanes, secrets — kept whatever it had loaded on entry. So the
verb most likely to change what you are looking at is the one whose
result you could not see, and the obvious reading of that screen is
"it did not work".

The current view now reloads after a command that completed, which is
one line and covers every list view at once.

**Coverage note, stated rather than glossed:** this fix lives in the
event loop, not the reducer, so no unit test covers it. What caught it
was driving the binary in a pty, and that is what would catch a
regression. The reducer and render tests remain the wrong shape for
"the screen did not reload".

## 5. An empty choice was called ambiguous (fixed)

Pressing Enter on a wizard's choice field with nothing typed:

```
'' is ambiguous: no, yes
```

Prefix matching treats an empty string as matching every option. Empty
input is not unclear, it is absent, and telling somebody their answer
was ambiguous when they have not given one sends them looking for the
wrong problem. Now `releasable is required: pick one of no, yes`.

Shared by every wizard with a choice field, so it was not new; the gate
wizard is just where somebody finally pressed Enter on one.

## Observed, not fixed

The gate wizard defaults the upstream field to the first known gate, and
"known" means the gate list the TUI has loaded. Open the wizard before
that arrives and the default is empty, which makes the new gate a second
*entry* gate rather than a stage. Legal, visible on the review step
(`upstream:` is blank), and still a surprise waiting for somebody in a
hurry. Worth a proper answer — probably refusing to guess rather than
guessing from a race — rather than a patch at the end of a drive.

## What held

Everything else driven in this pass behaved:

- `doctor --deep` names the staged release rather than the last-seen
  bundle, and `cached logins` reports live entries after the 493-file
  sweep
- publish, promote, approve, release ran entirely from inbox rows,
  pasted as printed
- a bundle with nowhere left to go leaves the queue rather than nagging
- the error unwrapping from 26.3 holds: every refusal in this pass was a
  sentence
- the TUI dashboard picked up the new `promote` recommendation and its
  ranking without changing a line of dashboard code, because 23.4 put the
  ordering inside `inbox_actions` rather than in the view
- the gate wizard's review step says `Enter: run` rather than naming a
  consequence, which is correct: `confirmation_prompt` names consequences
  for verbs that are hard to walk back, and adding a gate strands nothing
  and is undone by `gates rm`. Checked rather than assumed

## Next Task

None outstanding. `22.5` remains the operator's call.
