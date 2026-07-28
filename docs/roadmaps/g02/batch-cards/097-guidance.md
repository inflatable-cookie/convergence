# 097 Guidance

Status: complete
Updated: 2026-07-28
Roadmap: `g02.027`

## Objective

Every screen answers "what can I do here, and what would it do" without
being asked.

## Scope of the actual problem

27.1 gave navigation a visible surface and 27.3's root redesign (done
early, from the operator's screenshot) gave the dashboard a selectable
to-do list that explains its Enter. What remains is the legacy's
biggest loss: the suggestion data carries bare verb names with no help
text, and the console only shows them once you have started typing the
right letters. The list views also still show ids where they could show
state.

## In Scope

- help text restored to the suggestion data, verb by verb
- the console's suggestion panel always visible while the console is
  open, legacy-style: verb + what it does, selection highlighted
- list views showing state a person recognises (bundles: status and
  age, not just an id)

## Acceptance Criteria

- opening the console with `:` immediately shows what can be typed and
  what each does
- the operator's verdict, which is 27.4's job to collect

## Outcome

`COMMANDS` carries a one-line purpose per verb — the data the rebuild
dropped. Opening the console with `:` shows the whole menu immediately,
because the empty state is exactly when somebody needs it; typing
filters; the window scrolls with the highlight, since 37 verbs in nine
rows otherwise strand the selection off-screen. The Help screen lists
one verb per line with its purpose instead of a packed name grid.

One interaction settled on the way: the always-on menu stole bare Up
from history recall, so the rule is now *what you have typed* decides —
empty line recalls history, a part-typed verb moves the menu. A test
asserts every verb has help, so no row can render blank.

List-view interiors (bundles/lanes rows showing state) moved to 27.4's
drive: what those rows are missing is best named by the person reading
them cold.

## Next Task

Batch card 27.4 (somebody else drives it) — the operator, on a repo they
have not been walked through.
