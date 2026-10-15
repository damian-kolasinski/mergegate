# 14 — Prioritization

Roadmap row 14 · decisions settled 2026-10-15

## Scope

Items carry a priority and a "why" note (design.md §10), set by `mg submit --priority --note` on an item's first submit and changed any time with `mg prioritize` or the UI's `p` picker. The inbox orders every group by priority, then conflict pressure, then age. `mg events --follow` streams the hub-wide event log to the operator agent. The hub test-merges open items when their target moves, sends conflicts to their agents before the operator gets to them, and shows which open items conflict with each other. That's F10 and F9's early detection. Stacked children in the ordering and in conflicts come from step 13 (roadmap: "the one that lands second adds the meeting"), and the judge pass is 17.

## Decisions

1. **Two PRs:** priority, ordering and the event stream, then early conflict detection (Stack).
2. **`--priority/--note` count on the submit that creates the item only.** A later revision ignores them and warns, pointing at `mg prioritize`, so an agent repeating its brief can't undo a change the operator made.
3. **An early conflict moves the item to Changes requested** (With agents, as F9 draws it) with the existing `merge.conflict` event; its next clean revision brings it back to Needs you. Drafts stay.
4. **`mg events --follow` streams the whole log:** every item's events, hub-wide, in `mg wait`'s event shape, one JSON object per line. It starts from now, takes `--since CURSOR` to resume, and acknowledges nothing.
5. *(agent)* **No priority** sorts after p3 and shows no badge; `--priority none` clears it.
6. *(agent)* **`mg prioritize` writes priority and note together:** leaving out `--note` clears the old one. The latest write wins, CLI or UI. Neither the authoring agent nor GitHub hears about it.
7. *(agent)* **The UI sets priority with `p`** on an inbox row, and `P` on the review screen, where `p` is the previous file: p0–p3 or none, and a note. No dragging. The picker holds the keys while it's open.
8. *(agent)* **The target is watched** on every submit, after each of the hub's own merges, and by polling each project's target every 60s. Items in Merging aren't test-merged, and a conflict is sent once per revision.
9. *(agent)* **Pairwise conflicts** between open items of one project and target show in the inbox ("conflicts with app#45") and reach no agent: merging one turns it into a real conflict for the other.
10. *(agent)* **Order within every group:** priority, then how many open items it conflicts with (most first), then oldest first.

## Where it lives

- `core`: `item::priority` (the type, `prioritize`), ordering in `item::inbox`; `feedback` gains the hub-wide read for the stream. PR 2: test-merging open items, pairwise conflicts.
- `store-sqlite`: `0008_priority.sql` (priority and note on items). PR 2: a conflicts table.
- `api::item`, `api::feedback`; `hub::items`, a `hub::priority` route, the event stream route; `cli::prioritize_cmd`, `cli::events_cmd`, `submit_cmd`'s flags; the skill's operator and authoring references.
- UI: priority badges, notes and the picker in `ui/inbox`, the review header's badge.

## Stack

- `14-prioritization-1-priority`: priority and note, `mg prioritize`, the `p` picker, inbox ordering, `mg events --follow`, this brief. *In progress.*
- `14-prioritization-2-conflicts`: target watch and early `merge.conflict`, pairwise conflicts in the inbox, conflict pressure in the ordering; F9's early detection end to end, doc updates, deleting this brief and roadmap row 14. *Planned.*

## Open

- **13** adds stacked children to the ordering ("items stacked on it") and reports a conflict its parent also has on the parent only, if it lands after 14.
