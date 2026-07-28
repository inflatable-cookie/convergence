# 097 Guidance

Status: ready
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

## Next Task

Batch card 27.4 (somebody else drives it).
